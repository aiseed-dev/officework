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
    let parses = analyze(context);
    if parses.is_empty() {
        return Vec::new();
    }
    targets
        .iter()
        .map(|t| {
            let mut readings: Vec<String> = Vec::new();
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

/// 本文を N 通りに解析します。**1本も取れなければ空**を返します。
fn analyze(context: &str) -> Vec<Parse> {
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
        let 本文 = "人気のない路地を歩く";
        let t = [Target { base: "人気".into(), at: 0 }];
        let s = candidates(本文, &t);
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

    /// 位置で指すので、同じ語が二度出ても別々に答えられます
    #[test]
    fn 同じ語が二度出ても位置で分かれる() {
        if !辞書があるか() {
            return;
        }
        let 本文 = "山と山";
        let t = [
            Target { base: "山".into(), at: 0 },
            Target { base: "山".into(), at: "山と".len() },
        ];
        let s = candidates(本文, &t);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].at, 0);
        assert_eq!(s[1].at, "山と".len());
        assert!(s[0].readings.contains(&"やま".to_string()), "{:?}", s[0].readings);
        assert!(s[1].readings.contains(&"やま".to_string()), "{:?}", s[1].readings);
    }
}
