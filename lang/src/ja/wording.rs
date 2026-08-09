//! 重複表現(重言) — **同じ意味を二度言っている**所を見つける。
//!
//! 頭痛が痛い・まず最初に・各位様。表記ゆれと同じく**辞書もモデルも要らない**。
//! 有限の一覧で足りるからで、その一覧は**自分で書く**(決めごと3 —
//! 民間の校正基準や社内表記ルール集は写さない。免許が別)。
//!
//! # 難しいのは語彙ではなく許容の線引き
//!
//! 「まず最初に」は重言だが**日常的に使われている**。「各ページごと」も同じ。
//! ここで「誤り」と断定すると、正しく通じている文章に赤を入れることになって
//! 煩いだけになる。だから種別は [`Kind::Wording`](crate::check::Kind::Wording)
//! ——画面に出る言葉は**「言い回し」**であって「誤り」ではない。
//! 表記ゆれで採ったのと同じ作法で、**言い換えの案を出すだけ**にする。
//!
//! # 一覧に載せる基準
//!
//! **重なっている語が字面で見える物だけ**。「頭痛」と「痛い」のように、
//! 同じ字が二度出るか、意味の重なりが辞書を引かずに分かる物に限る。
//!
//! 排気ガス(排気=排出される気体)や製造メーカーのように、
//! **語源としては重言でも、日本語として定着し切っている物は載せない。**
//! 当たらない検査は無いより悪い。

use crate::check::{Finding, Kind, Source};

/// 重言1つ。
///
/// `then` が空なら `first` がそのまま並んでいるかを見る。
/// 空でなければ **`within` 字以内に続けて現れるか**を見る(頭痛 が 痛い)。
struct Rule {
    first: &'static str,
    then: &'static str,
    /// `first` と `then` の間に許す字数
    within: usize,
    /// 言い換えの案。**直せという意味ではない**
    suggest: &'static str,
    /// `first` が**単独の語**として立っているときだけ見るか。
    ///
    /// 「約」は熟語の中にいくらでも現れる(契**約**・要**約**・**約**物・**約**款)。
    /// 前後が漢字なら熟語の一部と見て見送る — ただし後ろが数の漢字なら
    /// 「約三日」なので数詞として通す。「今現在」のように熟語の顔で現れる
    /// 規則は `false`
    alone: bool,
    /// `first` の直後がこの字なら別の語。空なら歯止め無し。
    /// 「必ず**しも**要らない」は「必ず必要」ではない — 実測で出た誤検出
    unless: &'static str,
}

/// 文の切れ目。ここを跨いだら別の話をしている。
const BREAK: &str = "。！？!?\n";

const fn r(first: &'static str, then: &'static str, within: usize, suggest: &'static str) -> Rule {
    Rule { first, then, within, suggest, alone: false, unless: "" }
}

/// 直後が別の語になる形を外す規則(必ず**しも**)。
const fn unless(
    first: &'static str,
    then: &'static str,
    w: usize,
    suggest: &'static str,
    unless: &'static str,
) -> Rule {
    Rule { first, then, within: w, suggest, alone: false, unless }
}

/// 単独の語として立っているときだけ見る規則(約〜)。
const fn alone(first: &'static str, then: &'static str, w: usize, suggest: &'static str) -> Rule {
    Rule { first, then, within: w, suggest, alone: true, unless: "" }
}

/// **自分で書いた一覧。** 事務の文書で実際に出る物から選んだ。
const TABLE: &[Rule] = &[
    // --- 同じ字が二度出る ---
    // 「痛」「犯」だけを見ると 腹**痛**・**犯**人 を拾う(実測)。活用形まで書く
    r("頭痛", "痛い", 4, "頭が痛い"),
    r("頭痛", "痛む", 4, "頭が痛む"),
    // 「を」まで含める。犯罪**者**が自分の犯した罪、は重言ではない(実測)
    r("被害を", "被っ", 6, "被害を受ける"),
    r("被害を", "被る", 6, "被害を受ける"),
    r("犯罪を", "犯し", 6, "罪を犯す"),
    r("犯罪を", "犯す", 6, "罪を犯す"),
    r("違和感", "感じ", 4, "違和感がある"),
    // --- 重なりが字面で見える ---
    r("まず", "最初", 3, "最初に"),
    r("一番", "最初", 2, "最初"),
    r("一番", "最後", 2, "最後"),
    r("いちばん", "最初", 2, "最初"),
    r("今", "現在", 1, "現在"),
    r("現在", "現状", 3, "現状"),
    r("今", "現状", 2, "現状"),
    r("従来", "から", 0, "従来"),
    r("後で", "後悔", 3, "後悔"),
    r("あとで", "後悔", 3, "後悔"),
    r("予め", "予約", 4, "予約"),
    r("あらかじめ", "予約", 4, "予約"),
    unless("必ず", "必要", 3, "必要", "しも"),
    r("まだ", "未定", 3, "未定"),
    r("未だ", "未定", 3, "未定"),
    r("過半数", "超え", 3, "過半数に達する"),
    r("挙式", "挙げ", 4, "式を挙げる"),
    r("各位", "様", 0, "各位"),
    r("各位", "殿", 0, "各位"),
    r("諸", "先生方", 0, "先生方"),
    r("元旦", "の朝", 0, "元旦"),
    r("最後", "の切り札", 0, "切り札"),
    r("一番", "ベスト", 2, "ベスト"),
    r("お体", "ご自愛", 6, "ご自愛"),
    // --- 数の言い方 ---
    alone("約", "程度", 10, "「約」か「程度」のどちらか"),
    alone("約", "前後", 10, "「約」か「前後」のどちらか"),
    alone("約", "くらい", 10, "「約」か「くらい」のどちらか"),
    alone("約", "ぐらい", 10, "「約」か「ぐらい」のどちらか"),
];

fn is_kanji(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u) || u == 0x3005
}

/// 数を表す漢字。「約**三**日程度」の三は熟語の頭ではなく数詞。
const NUMERAL: &str = "〇一二三四五六七八九十百千万億兆半数";

/// `ch` の `i` から `s` が始まっているか。
fn starts_with(ch: &[char], i: usize, s: &str) -> bool {
    let t: Vec<char> = s.chars().collect();
    i + t.len() <= ch.len() && ch[i..i + t.len()] == t[..]
}

/// `first` が熟語の一部ではなく単独で立っているか。
fn standalone(ch: &[char], i: usize, len: usize) -> bool {
    if i > 0 && is_kanji(ch[i - 1]) {
        return false;
    }
    match ch.get(i + len) {
        // 後ろが漢字なら熟語(約物・約款)。ただし数の漢字は数詞(約三日)
        Some(&c) => !is_kanji(c) || NUMERAL.contains(c),
        None => true,
    }
}

/// `from` から `within` 字以内に `then` が現れるか。**文の切れ目は跨がない。**
fn follows(ch: &[char], from: usize, rule: &Rule) -> Option<usize> {
    let t: Vec<char> = rule.then.chars().collect();
    let limit = (from + rule.within).min(ch.len());
    let mut j = from;
    while j <= limit && j + t.len() <= ch.len() {
        if BREAK.contains(ch[j]) {
            return None;
        }
        if ch[j..j + t.len()] == t[..] {
            return Some(j + t.len());
        }
        j += 1;
    }
    None
}

/// 重複表現を探す。**辞書もモデルも要らない。**
pub fn findings(text: &str) -> Vec<Finding> {
    let ch: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    for rule in TABLE {
        let f: Vec<char> = rule.first.chars().collect();
        if f.is_empty() {
            continue;
        }
        let mut i = 0;
        while i + f.len() <= ch.len() {
            if ch[i..i + f.len()] != f[..] {
                i += 1;
                continue;
            }
            // 単独の語として立っているときだけ見る規則の歯止め。
            // 契**約**書 / **約**物 / **約**款 を「およそ」と読まない
            if rule.alone && !standalone(&ch, i, f.len()) {
                i += 1;
                continue;
            }
            if !rule.unless.is_empty() && starts_with(&ch, i + f.len(), rule.unless) {
                i += 1;
                continue;
            }
            let end = if rule.then.is_empty() {
                Some(i + f.len())
            } else {
                follows(&ch, i + f.len(), rule)
            };
            match end {
                Some(e) => {
                    out.push(Finding {
                        kind: Kind::Wording,
                        source: Source::Dictionary,
                        found: ch[i..e].iter().collect(),
                        at: Some(i),
                        candidates: vec![rule.suggest.to_string()],
                    });
                    i = e;
                }
                None => i += 1,
            }
        }
    }
    out.sort_by_key(|f| f.at.unwrap_or(usize::MAX));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<String> {
        findings(text).iter().map(|f| f.found.clone()).collect()
    }

    #[test]
    fn 同じ字を二度言っているのを見つける() {
        assert_eq!(found("頭痛が痛いので休みます。"), ["頭痛が痛い"]);
        assert_eq!(found("大きな被害を被った。"), ["被害を被っ"]);
        assert_eq!(found("違和感を感じた。"), ["違和感を感じ"]);
    }

    #[test]
    fn 離れていても続けて出れば拾う() {
        assert_eq!(found("まず最初にご説明します。"), ["まず最初"]);
        assert_eq!(found("まず、最初にご説明します。"), ["まず、最初"]);
    }

    #[test]
    fn 文の切れ目は跨がない() {
        // 別の話をしている。重言ではない
        assert!(found("まずご説明します。最初の点は…").is_empty());
        assert!(found("約100人が来ました。程度の差はあります。").is_empty());
    }

    #[test]
    fn 事務の文書で出る形() {
        assert_eq!(found("関係者各位様"), ["各位様"]);
        assert_eq!(found("約100名程度が参加します。"), ["約100名程度"]);
        assert_eq!(found("今現在の状況です。"), ["今現在"]);
        assert_eq!(found("従来から使っています。"), ["従来から"]);
        assert_eq!(found("必ず必要になります。"), ["必ず必要"]);
    }

    #[test]
    fn 熟語の一部を拾わない() {
        // 契**約**・要**約**・規**約** の 約 は「およそ」ではない
        assert!(found("契約書の程度を確認する。").is_empty());
        assert!(found("要約すると程度の問題だ。").is_empty());
        // 約物(やくもの)は組版の用語。「およそ物」ではない — 実測で出た誤検出
        assert!(found("約物の前後を詰める。").is_empty());
        assert!(found("約款の程度を確かめる。").is_empty());
        // 数の漢字は数詞なので通す
        assert_eq!(found("約三日程度かかります。"), ["約三日程度"]);
        // 前が漢字でなければ見る
        assert_eq!(found("約5日程度かかります。"), ["約5日程度"]);
    }

    #[test]
    fn 誤りとは言わない() {
        // 「まず最初に」は重言だが日常的に使われている。
        // 画面に出る言葉は「言い回し」で、直す先ではなく**案**を出すだけ
        let f = findings("まず最初にご説明します。");
        assert_eq!(f[0].kind, Kind::Wording);
        assert_eq!(f[0].kind.label(), "言い回し");
        assert_eq!(f[0].candidates, vec!["最初に".to_string()]);
    }

    #[test]
    fn 指摘は辞書の側から出る() {
        // モデルが居なくても出る = GPU 無しで再現できる
        assert!(findings("頭痛が痛い").iter().all(|f| f.source == Source::Dictionary));
    }

    #[test]
    fn 指摘の文字列は本文にそのまま在る() {
        let text = "まず最初に、関係者各位様へ約100名程度と伝えた。";
        for f in findings(text) {
            assert!(text.contains(&f.found), "本文に無い: {}", f.found);
        }
    }

    #[test]
    fn 似た形の別語を拾わない() {
        // どれも青空文庫711作品で実際に出た誤検出
        assert!(found("頭痛でも腹痛でもない。").is_empty(), "腹痛の痛を拾った");
        assert!(found("頭痛、歯痛、腰痛。").is_empty());
        assert!(found("犯罪者は犯人ではない。").is_empty(), "犯人の犯を拾った");
        assert!(found("犯罪者が、自分の犯した罪を悔いる。").is_empty(), "犯罪者を拾った");
        assert!(found("必ずしも今必要ではない。").is_empty(), "必ずしも を拾った");
        // 本物は残る
        assert_eq!(found("犯罪を犯した。"), ["犯罪を犯し"]);
        assert_eq!(found("必ず必要です。"), ["必ず必要"]);
    }

    #[test]
    fn 普通の文には何も出ない() {
        assert!(found("関係者各位").is_empty());
        assert!(found("最初にご説明します。").is_empty());
        assert!(found("約100名が参加します。").is_empty());
        assert!(found("現在の状況をご報告します。").is_empty());
        assert!(found("").is_empty());
    }

    #[test]
    fn 指摘は本文の順に並ぶ() {
        let f = findings("まず最初に。次に頭痛が痛い。");
        let ats: Vec<usize> = f.iter().filter_map(|x| x.at).collect();
        assert!(ats.windows(2).all(|w| w[0] <= w[1]), "{ats:?}");
    }
}
