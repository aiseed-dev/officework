//! **辞書で読みの候補を出す**(2026-08-20 発注者「ふりがなは AI を使わずに
//! 日本語辞書でやろうとしていた」)。
//!
//! [`super::furigana`] と同じ `Suggestion` を返します。違うのは出どころだけで、
//! **辞書は候補の範囲を決め、モデルは並べ替える**という分担です。
//!
//! # なぜ辞書だけでは終わらないか
//!
//! 青空文庫の人手ルビで測りました(`tools/furigana_dict_bench.py`。6作583箇所)。
//! 現代語の1作で 76.1%、旧仮名の混ざる全体で 50.4% です。外れは3つに分かれます。
//!
//! . 旧仮名遣い(老婆《らうば》に対して辞書は「ろうば」)— *読みは合っている*
//! . **同じ語で読みが割れる**(己《おれ》↔おのれ、扉《と》↔とびら)— ここがモデルの持ち場
//! . 辞書に無い(熟字訓・当て字)
//!
//! 2 は辞書が**両方持っている**ことが多く、順序だけが違います。だから
//! 候補を並べて渡せば、モデルは選ぶだけで済みます。
//!
//! # 外の道具に頼ります(組む時ではなく、使う時)
//!
//! `mecab` が居なければ [`available`] が偽を返し、[`candidates`] は空を返します。
//! **黙って間違えるより、無いと言うほうがよい**ので、当て推量はしません。
//! 辞書の置き場は `OFFICEWORK_MECAB_DIC` で名指しできます。

use crate::Target;

use super::furigana::Suggestion;

/// N-best をいくつ取るか。**候補の数の上限**です。
///
/// 10 は測って決めました — 人手ルビが入っていた最も深い順位は2位で、
/// 10 まで見れば取りこぼしはありませんでした。増やすほど遅くなります。
const NBEST: usize = 10;

/// 辞書が使えるか。**組む時ではなく使う時に見ます。**
pub fn available() -> bool {
    std::process::Command::new("mecab")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 辞書の置き場(名指しがあればそれ)。無ければ機械の既定に任せます。
fn dic() -> Option<String> {
    std::env::var("OFFICEWORK_MECAB_DIC").ok().filter(|s| !s.is_empty())
}

/// カタカナをひらがなに。**比べ方をそろえるため。**
/// 辞書はカタカナで返し、ルビはひらがなで書かれます。
pub fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| {
            let o = c as u32;
            if (0x30A1..=0x30F6).contains(&o) {
                char::from_u32(o - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// 解析1本ぶん。(本文での位置, 表層, 読み)
type Parse = Vec<(usize, String, String)>;

/// **指定した語の読み候補を、辞書から出す。**
///
/// `context` には前後を含めた本文を渡します。同じ語でも位置で答えが変わるので、
/// `targets` は位置(`at`)で指します。
///
/// 候補の順は**辞書が尤もらしいと見た順**です(N-best の1本目が先頭)。
/// これはそのまま答えではありません — 測ったところ、1位が違って2位が
/// 正しい場面がありました(人気《ひとけ》)。**並べ替えはモデルの仕事**です。
pub fn candidates(context: &str, targets: &[Target]) -> Vec<Suggestion> {
    if targets.is_empty() || context.is_empty() || !available() {
        return Vec::new();
    }
    let 行ごと = analyze_all(context);
    if 行ごと.is_empty() {
        return Vec::new();
    }
    targets
        .iter()
        .map(|t| {
            let mut readings: Vec<String> = Vec::new();
            // その語が居る行の解析だけを見ます
            let parses: &[Parse] = 行ごと
                .iter()
                .find(|ps| ps.first().is_some_and(|p| p.iter().any(|(a, _, _)| *a == t.at)))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            for (k, p) in parses.iter().enumerate() {
                // **2本目からは、語がまるごと1つで取れたときだけ採ります。**
                // 下位の解析は*語の分かれ方*が違うだけで、辞書が持つ別の
                // 読みではありません。混ぜると「路地」に「みちち」「ろち」が
                // 並びます(2026-08-20 に実際に出た)。
                // 分かれた熟語を拾うのは1本目だけの仕事です
                if let Some(y) = reading_at(p, t.at, &t.base, k == 0) {
                    if !y.is_empty() && !readings.contains(&y) {
                        readings.push(y);
                    }
                }
            }
            Suggestion { base: t.base.clone(), at: t.at, readings }
        })
        .collect()
}

/// 行ごとに解析して、**本文全体での位置**に直します。
///
/// 返すのは「解析の並び」の並び — 行ごとに N 本ぶんです。
fn analyze_all(context: &str) -> Vec<Vec<Parse>> {
    let mut out = Vec::new();
    let mut 行頭 = 0usize;
    for line in context.split('\n') {
        if !line.trim().is_empty() {
            let mut ps = analyze_line(line);
            for p in ps.iter_mut() {
                for t in p.iter_mut() {
                    t.0 += 行頭;
                }
            }
            out.push(ps);
        }
        行頭 += line.len() + 1; // 改行のぶん
    }
    out
}

/// 漢字を含むか。**ふりがなを振る相手を選ぶのに使います。**
///
/// ひらがな・カタカナ・英数字だけの語には振りません。
pub fn has_kanji(s: &str) -> bool {
    s.chars().any(|c| {
        let o = c as u32;
        // CJK 統合漢字と拡張A、それに互換漢字。々(踊り字)も漢字の扱い
        (0x4E00..=0x9FFF).contains(&o)
            || (0x3400..=0x4DBF).contains(&o)
            || (0xF900..=0xFAFF).contains(&o)
            || o == 0x3005
    })
}

/// **本文の中の、ふりがなを振れる語を全部拾う。**
///
/// 返すのは `(位置, 語, 読みの候補)` で、候補が2つ以上ある語は
/// *読みが割れています* — そこがモデルに訊く相手です。
///
/// 読みが語と同じ(ひらがなの語など)物と、漢字を含まない物は外します。
pub fn ruby_targets(context: &str) -> Vec<Suggestion> {
    if context.is_empty() || !available() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for parses in analyze_all(context) {
        let Some(first) = parses.first() else { continue };
        for (at, surf, yomi) in first {
            if !has_kanji(surf) || yomi.is_empty() {
                continue;
            }
            let mut readings = vec![yomi.clone()];
            // 2本目からは、同じ位置に**同じ表層で**別の読みが立つときだけ足します
            for p in parses.iter().skip(1) {
                if let Some((_, s2, y2)) = p.iter().find(|(a, _, _)| a == at) {
                    if s2 == surf && !y2.is_empty() && !readings.contains(y2) {
                        readings.push(y2.clone());
                    }
                }
            }
            out.push(Suggestion { base: surf.clone(), at: *at, readings });
        }
    }
    out
}

/// **1行**を N 通りに解析します。位置はその行の中の位置です。
///
/// *行ごとに呼びます。* mecab は行を1つの文として扱うので、改行を含む字を
/// そのまま渡すと行の数だけ解析が出て、位置も行ごとに0へ戻ります。
/// まとめて渡して数で切り分けようとして失敗しました(2026-08-20)—
/// `-N` は「最大N個」で、行によって本数が変わるため境が分かりません。
/// **実機で見て気づきました**: 見出しと本文の2行の文書で、見出しにだけ
/// ふりがなが付きました
fn analyze_line(context: &str) -> Vec<Parse> {
    let mut cmd = std::process::Command::new("mecab");
    if let Some(d) = dic() {
        cmd.arg("-d").arg(d);
    }
    cmd.arg("-N").arg(NBEST.to_string());
    let out = match run(cmd, context) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut parses: Vec<Parse> = Vec::new();
    let mut cur: Parse = Vec::new();
    // **位置は表層を順に足して出します。** 解析は入力の字をそのまま並べるので、
    // 頭から探していけば同じ字が何度出てきてもずれません
    let mut at = 0usize;
    for line in out.lines() {
        if line == "EOS" {
            if !cur.is_empty() {
                parses.push(std::mem::take(&mut cur));
            }
            at = 0;
            continue;
        }
        let Some((surf, rest)) = line.split_once('\t') else { continue };
        let f: Vec<&str> = rest.split(',').collect();
        let yomi = f.get(7).filter(|x| **x != "*").map(|x| kata_to_hira(x)).unwrap_or_default();
        let Some(found) = context[at..].find(surf).map(|i| at + i) else { continue };
        cur.push((found, surf.to_string(), yomi));
        at = found + surf.len();
    }
    if !cur.is_empty() {
        parses.push(cur);
    }
    parses
}

/// その位置から始まる `base` の読み。**取れなければ None。**
///
/// 青空のルビは漢字の部分にだけ付きます(呟《つぶや》く)。辞書は送り仮名
/// 込みの語(呟く)で持つので、**語が親字で始まるときは頭の分だけ**を見ます。
///
/// `分かれた熟語も` が偽なら、語がまるごと1つで取れたときだけ返します。
/// 下位の解析から拾うと、別の分かれ方の読みが候補に混ざります。
fn reading_at(p: &Parse, at: usize, base: &str, 分かれた熟語も: bool) -> Option<String> {
    let i = p.iter().position(|(a, _, _)| *a == at)?;
    let (_, surf, yomi) = &p[i];
    if surf == base {
        return Some(yomi.clone());
    }
    if surf.starts_with(base) {
        // 送り仮名つき。**読みの頭を切り出せないので、丸ごと返します** —
        // 呼ぶ側が「ルビで始まるか」で見ます。ここで当て推量に切ると、
        // 「湛える(たたえる)」を「たた」と決めつけることになります
        return Some(yomi.clone());
    }
    // 親字が複数の語にまたがる(熟語が分かれた場合)
    if !分かれた熟語も {
        return None;
    }
    let mut s = String::new();
    let mut y = String::new();
    for (_, sf, ym) in &p[i..] {
        s.push_str(sf);
        y.push_str(ym);
        if s.len() >= base.len() {
            break;
        }
    }
    (s == base || s.starts_with(base)).then_some(y)
}

fn run(mut cmd: std::process::Command, input: &str) -> Option<String> {
    use std::io::Write as _;
    let mut ch = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    ch.stdin.as_mut()?.write_all(input.as_bytes()).ok()?;
    ch.stdin.as_mut()?.write_all(b"\n").ok()?;
    drop(ch.stdin.take());
    let out = ch.wait_with_output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 辞書が無い機械では飛ばします。**CI を落とさない** — `mecab` は
    /// 組む時ではなく使う時の道具なので、無いことは欠陥ではありません
    fn 辞書があるか() -> bool {
        if available() {
            return true;
        }
        eprintln!("mecab が無いので飛ばします(sudo apt install mecab mecab-ipadic-utf8)");
        false
    }

    #[test]
    fn カタカナをひらがなにする() {
        assert_eq!(kata_to_hira("ソノゴ"), "そのご");
        assert_eq!(kata_to_hira("ヒトケ"), "ひとけ");
        // ひらがなと記号はそのまま
        assert_eq!(kata_to_hira("あいう、"), "あいう、");
        // 長音はカタカナの範囲の外なので残る
        assert_eq!(kata_to_hira("コーヒー"), "こーひー");
    }

    #[test]
    fn 辞書が無ければ空を返す() {
        // 語を指していなければ、辞書の有無によらず空
        assert!(candidates("本文です", &[]).is_empty());
        assert!(candidates("", &[Target { base: "本".into(), at: 0 }]).is_empty());
    }

    /// **割れる語で候補が2つ以上出る。** ここが分担の前提です —
    /// 辞書が両方持っていなければ、モデルが並べ替えても届きません
    #[test]
    fn 割れる語は候補が複数出る() {
        if !辞書があるか() {
            return;
        }
        let body = "人気のない路地を歩く";
        let t = [Target { base: "人気".into(), at: 0 }];
        let s = candidates(body, &t);
        assert_eq!(s.len(), 1);
        let r = &s[0].readings;
        assert!(r.contains(&"にんき".to_string()), "にんき が無い: {r:?}");
        assert!(r.contains(&"ひとけ".to_string()), "ひとけ が無い: {r:?}");
        // **順序は答えではありません。** 1位が違うことがあるので、
        // 何位にあったかだけを見ます
        assert!(s[0].rank_of("ひとけ").is_some());
    }

    /// 割れない語は候補が1つ。**この語はモデルに訊かなくてよい**という印です
    #[test]
    fn 割れない語は候補が1つ() {
        if !辞書があるか() {
            return;
        }
        let s = candidates("路地を歩く", &[Target { base: "路地".into(), at: 0 }]);
        assert_eq!(s[0].readings, vec!["ろじ".to_string()], "{:?}", s[0].readings);
    }

    #[test]
    fn 漢字を含むかを見分ける() {
        assert!(has_kanji("報告書"));
        assert!(has_kanji("行った"));
        assert!(has_kanji("人々"));
        assert!(!has_kanji("ひらがな"));
        assert!(!has_kanji("カタカナ"));
        assert!(!has_kanji("ABC123"));
        assert!(!has_kanji("、。"));
    }

    /// **振る相手を拾う。** 漢字を含む語だけが出て、割れる語は候補が複数
    #[test]
    fn 振る相手を拾う() {
        if !辞書があるか() {
            return;
        }
        let v = ruby_targets("人気のない路地を歩く");
        let word: Vec<&str> = v.iter().map(|s| s.base.as_str()).collect();
        assert!(word.contains(&"人気"), "{word:?}");
        assert!(word.contains(&"路地"), "{word:?}");
        // ひらがなだけの語(の・ない・を)は入らない
        assert!(!word.contains(&"の"), "{word:?}");
        let 人気 = v.iter().find(|s| s.base == "人気").expect("人気 が無い");
        assert!(人気.readings.len() >= 2, "割れているのに候補が1つ: {:?}", 人気.readings);
        let 路地 = v.iter().find(|s| s.base == "路地").expect("路地 が無い");
        assert_eq!(路地.readings, vec!["ろじ".to_string()]);
    }

    /// **改行をまたいでも位置がずれない**(2026-08-20 に実機で踏んだ)。
    ///
    /// mecab は行を1つの文として扱います。まとめて渡していたので、
    /// 見出しと本文の2行の文書では**見出しにだけ**ふりがなが付き、
    /// 位置も行ごとに0へ戻っていました。
    #[test]
    fn 改行をまたいでも位置が合う() {
        if !辞書があるか() {
            return;
        }
        let body = "報告書\n人気のない路地を歩く";
        let v = ruby_targets(body);
        let word: Vec<&str> = v.iter().map(|s| s.base.as_str()).collect();
        // 見出しは「報告」+「書」に切れます(辞書の切り方。誤りではない)
        assert!(word.contains(&"報告"), "1行目が拾えていない: {word:?}");
        assert!(word.contains(&"人気"), "2行目が拾えていない: {word:?}");
        assert!(word.contains(&"路地"), "2行目が拾えていない: {word:?}");
        // **位置が本文の中の位置になっているか。** 切り出して語と合うかで見る
        for s in &v {
            assert_eq!(
                &body[s.at..s.at + s.base.len()],
                s.base,
                "位置がずれている: {} が {} を指している",
                s.base,
                &body[s.at..s.at + s.base.len()]
            );
        }
    }

    /// 位置で指すので、同じ語が二度出ても別々に答えられます
    #[test]
    fn 同じ語が二度出ても位置で分かれる() {
        if !辞書があるか() {
            return;
        }
        let body = "山と山";
        let t = [
            Target { base: "山".into(), at: 0 },
            Target { base: "山".into(), at: "山と".len() },
        ];
        let s = candidates(body, &t);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].at, 0);
        assert_eq!(s[1].at, "山と".len());
        assert!(s[0].readings.contains(&"やま".to_string()), "{:?}", s[0].readings);
        assert!(s[1].readings.contains(&"やま".to_string()), "{:?}", s[1].readings);
    }
}
