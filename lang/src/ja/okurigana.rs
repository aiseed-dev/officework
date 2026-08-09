//! 送り仮名 — 内閣告示「送り仮名の付け方」(昭和48年内閣告示第2号)による。
//!
//! **法令・国の告示は著作権の保護の外**(著作権法13条)なので、そのまま典拠にできる
//! (決めごと3)。民間の校正基準や社内の表記ルール集は写さない。
//!
//! # ここが出せる指摘は**狭い**
//!
//! 送り仮名には**本則・許容・例外の3段**がある。「行なう」は通則1の**許容**で、
//! **誤りではない**(決めごと4「許容を誤りにしない」)。「申込む」も通則6の許容。
//! だから「規則に無いから誤り」とは言えず、機械が断定すると
//! **正しい文章に赤が入って信用を失う。**
//!
//! 出せるのは2つだけ:
//!
//! | | 何を言うか | 断定するか |
//! |---|---|---|
//! | [`WRONG`] | 本則でも許容でも例外でもない形(少い → 少ない) | する |
//! | [`ALLOWED`] | 本則と許容が**同じ文書に混ざっている**(行う と 行なう) | **しない** |
//!
//! **狭くてよい。** 当たらない検査は無いより悪い。
//!
//! # 表記ゆれ(1)と二重に出さない
//!
//! [`crate::ja::notation`] は骨格が**漢字2字以上**の語だけを見る。
//! こちらの [`ALLOWED`] は**漢字1字の語幹**だけを持つ。
//! だから同じ語を二度指摘することは無い — 境目を表で分けてある。

use crate::check::{Finding, Kind, Source};

/// **本則でも許容でも例外でもない形。** ここだけは直す先を言ってよい。
///
/// 通則1の例外(明るい・少ない・冷たい・平たい・危ない)と、
/// 語幹が「し」で終わる形容詞(新しい・珍しい・難しい・著しい)から採った。
/// **「大い」は入れない** — 「大いに」は通則5の例外で正しい。
///
/// 3つ目は「直後がこの字なら別の語」の歯止め。**「穴が明いた」は動詞の
/// 「明く」**で、形容詞の「明るい」ではない(青空文庫の実測で出た誤検出)。
const WRONG: &[(&str, &str, &str)] = &[
    ("少い", "少ない", ""),
    ("危い", "危ない", ""),
    ("明い", "明るい", "たて"),
    ("冷い", "冷たい", ""),
    ("平い", "平たい", ""),
    ("暖い", "暖かい", ""),
    ("短かい", "短い", ""),
    ("新らしい", "新しい", ""),
    ("珍らしい", "珍しい", ""),
    ("難かしい", "難しい", ""),
    ("著るしい", "著しい", ""),
    ("恥かしい", "恥ずかしい", ""),
    ("悔やしい", "悔しい", ""),
    ("幸わせ", "幸せ", ""),
    ("味あう", "味わう", ""),
    ("承わる", "承る", ""),
    ("全たく", "全く", ""),
    ("快よい", "快い", ""),
];

/// **本則と許容の組。どちらも正しい。**
///
/// 通則1の許容(活用語尾の前の音節から送る)と通則2の許容(読み間違える
/// おそれのない場合は省ける)から採った。**混ざっているときだけ言い、
/// どちらが正しいとは言わない** — 表記ゆれ(1)と同じ作法。
///
/// 語幹が漢字1字の物だけを載せる。2字以上は 1 の担当で、二重に出さないため。
const ALLOWED: &[(&str, &str)] = &[
    // 通則1 許容
    ("行う", "行なう"),
    ("表す", "表わす"),
    ("著す", "著わす"),
    ("現れる", "現われる"),
    ("現す", "現わす"),
    ("断る", "断わる"),
    ("賜る", "賜わる"),
    // 通則2 許容
    ("浮かぶ", "浮ぶ"),
    ("生まれる", "生れる"),
    ("押さえる", "押える"),
    ("捕らえる", "捕える"),
    ("積もる", "積る"),
    ("聞こえる", "聞える"),
    ("当たる", "当る"),
    ("終わる", "終る"),
    ("変わる", "変る"),
];

fn is_kanji(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u) || u == 0x3005
}

/// `needle` が現れる文字位置を全部拾う。
///
/// **前が漢字なら熟語の一部**なので飛ばす — 「銀**行う**んぬん」を「行う」と
/// 読んだり、「代**表す**る」を「表す」と読んだりしないため。
fn occurrences(ch: &[char], needle: &str) -> Vec<usize> {
    let n: Vec<char> = needle.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + n.len() <= ch.len() {
        if ch[i..i + n.len()] == n[..] && !(i > 0 && is_kanji(ch[i - 1])) {
            out.push(i);
            i += n.len();
        } else {
            i += 1;
        }
    }
    out
}

/// 送り仮名の指摘。**辞書もモデルも要らない。**
pub fn findings(text: &str) -> Vec<Finding> {
    let ch: Vec<char> = text.chars().collect();
    let mut out = Vec::new();

    // 本則でも許容でも例外でもない形。**直す先を言ってよい**
    for (bad, good, unless) in WRONG {
        for at in occurrences(&ch, bad) {
            // 直後が別の語を作る字なら見送る(穴が明**いた** は動詞)
            if ch.get(at + bad.chars().count()).is_some_and(|c| unless.contains(*c)) {
                continue;
            }
            out.push(Finding {
                kind: Kind::Okurigana,
                source: Source::Dictionary,
                found: (*bad).to_string(),
                at: Some(at),
                candidates: vec![(*good).to_string()],
            });
        }
    }

    // 本則と許容の混在。**どちらが正しいとは言わない**(決めごと4・6)
    for (honsoku, kyoyo) in ALLOWED {
        let (a, b) = (occurrences(&ch, honsoku), occurrences(&ch, kyoyo));
        let (Some(&first_a), Some(&first_b)) = (a.first(), b.first()) else {
            continue;
        };
        out.push(Finding {
            kind: Kind::Okurigana,
            source: Source::Dictionary,
            found: (*honsoku).to_string(),
            at: Some(first_a),
            candidates: vec![(*kyoyo).to_string()],
        });
        out.push(Finding {
            kind: Kind::Okurigana,
            source: Source::Dictionary,
            found: (*kyoyo).to_string(),
            at: Some(first_b),
            candidates: vec![(*honsoku).to_string()],
        });
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
    fn 本則にも許容にも無い形は直す先を言う() {
        let f = findings("少い人数で行います。");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].found, "少い");
        assert_eq!(f[0].candidates, vec!["少ない".to_string()]);
        assert_eq!(f[0].kind, Kind::Okurigana);
        assert_eq!(found("珍らしい例です。"), ["珍らしい"]);
        assert_eq!(found("短かい期間でした。"), ["短かい"]);
    }

    #[test]
    fn 許容を誤りにしない() {
        // **決めごと4。** 行なう は通則1の許容で、誤りではない
        assert!(found("会議を行なう。").is_empty(), "許容を誤りにした");
        assert!(found("会議を行う。").is_empty());
        // 通則6の許容。申込む も誤りではない(1 の担当でもある)
        assert!(found("書面で申込む。").is_empty());
        // 通則5の例外。大いに は正しい
        assert!(found("大いに助かりました。").is_empty());
        // 穴が明いた は動詞「明く」。形容詞の「明るい」ではない(実測で出た誤検出)
        assert!(found("腰かけに穴が明いた。").is_empty(), "動詞の明くを拾った");
        assert!(found("穴が明いている。").is_empty());
        assert_eq!(found("明い部屋です。"), ["明い"]);
    }

    #[test]
    fn 熟語といたしますを拾わない() {
        // ご説**明い**たします を「明い」と読まない
        assert!(found("ご説明いたします。証明いたしました。").is_empty());
        assert!(found("減少いたしました。").is_empty());
    }

    #[test]
    fn 本則と許容が混ざっていたら言う() {
        // **どちらが正しいとは言わない。** 混ざっていることだけ
        let f = findings("会議を行う。作業は行なう。");
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0].found, "行う");
        assert_eq!(f[0].candidates, vec!["行なう".to_string()]);
        assert_eq!(f[1].found, "行なう");
        assert_eq!(f[1].candidates, vec!["行う".to_string()]);
    }

    #[test]
    fn 片方だけなら言わない() {
        // 揃っていれば、どちらに揃っていても正しい
        assert!(found("表す。表す。").is_empty());
        assert!(found("表わす。表わす。").is_empty());
        assert_eq!(found("表す。また表わす。").len(), 2);
    }

    #[test]
    fn 熟語の一部を拾わない() {
        // 銀**行う**んぬん / 代**表す**る を語として読まない
        assert!(found("銀行うんぬんの話。").is_empty());
        assert!(found("代表する立場。").is_empty());
        assert!(found("代表する。代表わす。").len() <= 1, "熟語を拾った");
    }

    #[test]
    fn 表記ゆれと二重に出さない() {
        // 1(表記ゆれ)は骨格が漢字2字以上、こちらは語幹が漢字1字。境目が分けてある
        let text = "問合せと問い合わせ。会議を行う。作業は行なう。";
        let n = crate::ja::notation::findings(text);
        let o = findings(text);
        assert!(!n.is_empty() && !o.is_empty(), "どちらかが出ていない");
        for a in &n {
            for b in &o {
                assert_ne!(a.found, b.found, "同じ語を二度指摘している: {}", a.found);
            }
        }
    }

    #[test]
    fn 指摘は辞書の側から出る() {
        assert!(findings("少い").iter().all(|f| f.source == Source::Dictionary));
    }

    #[test]
    fn 指摘の文字列は本文にそのまま在る() {
        let text = "少い人数。会議を行う。作業は行なう。";
        for f in findings(text) {
            assert!(text.contains(&f.found), "本文に無い: {}", f.found);
        }
    }

    #[test]
    fn 普通の文には何も出ない() {
        assert!(found("会議を行います。少ない人数で短い期間でした。").is_empty());
        assert!(found("").is_empty());
        assert!(found("English only.").is_empty());
    }

    #[test]
    fn 指摘は本文の順に並ぶ() {
        let f = findings("少い。表す。また表わす。珍らしい。");
        let ats: Vec<usize> = f.iter().filter_map(|x| x.at).collect();
        assert!(ats.windows(2).all(|w| w[0] <= w[1]), "{ats:?}");
    }
}
