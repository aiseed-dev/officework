//! 表記ゆれ — **同じ語が1つの文書の中で二通りに書かれている**のを見つける。
//!
//! この検査だけは**辞書もモデルも要らない**。証拠が文書の中で閉じているから。
//! 問合せ/問い合わせ、引渡し/引き渡し、申込/申し込み — どれも
//! 「文書の外の正解」を持ち出さずに、混ざっていることだけを言えばよい。
//!
//! **GPU の無い機械で日本語の校正が動き出す最初の一歩。**
//!
//! # 束ね方 — 骨格キー
//!
//! 形態素解析は使わない。日本語の事務文書で主に起きるのは**送り仮名の揺れ**で、
//! これは形が決まっている: **漢字の骨は動かず、間と後ろの仮名だけが変わる。**
//!
//! ```text
//! 問合せ ・ 問い合わせ ・ 問合わせ  →  骨格 問合
//! 引渡し ・ 引き渡し               →  骨格 引渡
//! 申込   ・ 申し込み               →  骨格 申込
//! ```
//!
//! 骨格で束ね、束の中に**書き方が2通り以上あれば**混在として報告する。
//!
//! # 断定しない
//!
//! **どちらが正しいとは言わない**(SEKKEI 決めごと6「誤検出は見逃しより重い」)。
//! 出すのは「この文書ではこの2つが混ざっている」だけで、選ぶのは利用者。
//! 引用の中の契約文言と本文とで送り仮名が違う、というのは正当に起こるので、
//! **断定しなければ誤検出の害はそこで止まる。**
//!
//! # 骨格キーだけでは束ね過ぎる(実測して足した歯止め)
//!
//! 素の骨格キーを青空文庫711作品に掛けたら、**関係の無い語まで束ねた**。
//! 落とした型と、落とすのに使った規則:
//!
//! | 束ね過ぎ | 例 | 規則 |
//! |---|---|---|
//! | 活用の途中で切る | 取り込み/取り込(む) | 語の後ろに助詞でない仮名 → 捨てる |
//! | 形容詞+名詞の句 | 若者/若い者・青空/青き空 | [`adjective_glue`] |
//! | 活用形どうし | 発売/発売し・思い出/思い出し | 送り仮名は前から落ちる([`drops_from_front`]) |
//! | サ変名詞に「し」 | 確認/確認し | 内側の送り仮名を要求する |
//!
//! **最後に残った束ね過ぎ**は、骨格が同じで**読みが違う**漢語と和語
//! (置換(ちかん)/置き換え(おきかえ)、独身/独り身)。[`KANGO`] で閉じた。

use crate::check::{Finding, Kind, Source};

/// 連用形の送り仮名になる仮名(い段・え段)。
///
/// 助詞と重なる **に・て・で・へ・ね** は入れない(「東京に行く」を
/// 1語として食ってしまうため)。歴史的仮名遣いの **ひ・ぢ・ぴ** も入れない
/// (現代の事務文書には出ず、青空文庫では誤検出だけを増やした)。
const OKURI: &str = "いきぎしじちびみりえけげせべめれ";

/// 2文字の送り仮名。**頭の1文字(わ・さ・も)は単独では送り仮名にならない**ので、
/// 対で持つ。「折れ線も」の も を食うと語尾が助詞になってしまう。
const HEAD2: [&str; 4] = ["わせ", "わり", "さえ", "もり"];

/// 語の切れ目として認める平仮名(助詞)。**これ以外の平仮名が続いていたら、
/// 活用の途中で切ったということ**なので、その候補ごと捨てる。
const PARTICLE: &str = "はがのにへとやもをかでねよ";

/// 助詞に見えて活用の途中である仮名と、その後ろに立つ活用語尾。
/// 「召し上がる」の が を助詞と読むと「召し上」で切れてしまう。
const FAKE_PARTICLE: &str = "がかで";
/// **「ら」は入れない** — 「入口から」の から を活用と読んでしまう。
/// 助詞としての出現がはるかに多い
const INFLECTION: &str = "るっりれろ";

/// 形容詞の連体形の語尾。**若い者/若者・青き空/青空**のように、
/// 名詞と繋がると複合語と見分けが付かなくなる。
const ADJ_TAIL: &str = "いじき";

/// **音読みで読む漢語。** 送り仮名を省いた和語と骨格が同じになる。
///
/// 置換(ちかん)は 置き換え(おきかえ)とは**別の語**だが、骨格はどちらも `置換`。
/// 切替(きりかえ)/切り替え とは**構造がまったく同じ**なので、字面では分けられない。
///
/// **常用漢字表の音訓では解けない。** 置(チ/お-く)も換(カン/か-える)も
/// 音訓を両方持つので、在庫を引いても「置換はチカンと読む語である」は出てこない。
/// それを言うには熟語の辞書が要り、それは決めごと2が断った外部の辞書になる。
/// だから**自分で書く**(決めごと3。民間の校正基準は写さない)。
///
/// **載せてよいのは、その2字が音読みの熟語として実在する物だけ。**
/// 切替・取扱・申込・引渡・受付は**和語**なので載せない — 載せると
/// 本物の表記ゆれが出なくなる(試験で縛ってある)。
const KANGO: &[&str] = &[
    "置換", "独身", "溺死", "通夜", "生物", "見物", "知人", "岐路", "清酒", "露出",
    "成功", "知己", "遊戯", "伸縮", "群集", "過去", "強引", "灯火", "燈火", "善悪",
    "乗馬", "乘馬", "帰途", "往来", "競馬", "大陸", "猛獣", "深紅", "亡父", "新芽",
    "独言", "名代", "人気", "心中", "大家", "一行", "生花", "工夫", "後生", "上手",
];

fn is_kanji(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u) || u == 0x3005 || (0xF900..=0xFAFF).contains(&u)
}

fn is_hiragana(c: char) -> bool {
    (0x3041..=0x309F).contains(&(c as u32))
}

/// 切り出した語1つ。`segs` は**漢字1字ごと**に「その字の後ろの送り仮名」を持つ。
///
/// 申込 → `[(申,""),(込,"")]` / 申し込み → `[(申,"し"),(込,"み")]`
/// と段の数が揃うので、そのまま突き合わせられる。
#[derive(Debug, Clone, PartialEq)]
struct Word {
    /// 本文における文字位置(バイトではない)
    at: usize,
    /// 本文にそのまま現れる文字列
    surface: String,
    segs: Vec<(char, String)>,
}

impl Word {
    /// 送り仮名を落とした漢字の骨。**これが束ねる鍵。**
    fn skeleton(&self) -> String {
        self.segs.iter().map(|(k, _)| *k).collect()
    }

    /// 漢字と漢字の**間**に送り仮名があるか。
    ///
    /// 複合動詞(問**い**合わせ・申**し**込み)の目印であり、
    /// 「確認」と「確認し」のような**サ変名詞の活用**を落とすのに使う。
    fn has_interior(&self) -> bool {
        self.segs[..self.segs.len() - 1].iter().any(|(_, o)| !o.is_empty())
    }
}

/// 送り仮名として読めるだけ読む(最大2文字)。
fn okurigana(ch: &[char], i: usize) -> String {
    let mut k = String::new();
    let mut j = i;
    while j < ch.len() && is_hiragana(ch[j]) && k.chars().count() < 2 {
        let c = ch[j];
        let two: String = ch[j..(j + 2).min(ch.len())].iter().collect();
        // 1文字目は送り仮名そのものか、2文字の送り仮名の頭(わせ・もり)。
        // 2文字目は送り仮名だけ
        let readable = if k.is_empty() {
            OKURI.contains(c) || HEAD2.contains(&two.as_str())
        } else {
            OKURI.contains(c)
        };
        if !readable {
            break;
        }
        k.push(c);
        j += 1;
    }
    k
}

/// 語の終わりとして認めてよい場所か。
///
/// **助詞でない平仮名が続いていたら、活用の途中で切っている。**
/// 「取り込む」を「取り込」として拾うと「取り込み」と混ざって見える。
fn boundary(ch: &[char], i: usize) -> bool {
    if i >= ch.len() {
        return true;
    }
    if is_hiragana(ch[i]) && !PARTICLE.contains(ch[i]) {
        return false;
    }
    // 助詞の顔をした活用語尾(召し上**がる**・見付**かる**)
    if i + 1 < ch.len() && FAKE_PARTICLE.contains(ch[i]) && INFLECTION.contains(ch[i + 1]) {
        return false;
    }
    true
}

/// 本文から候補の語を切り出す。
///
/// 形は **漢字+ ( 送り仮名 漢字+ )\* 送り仮名?** で、漢字が2字以上ある物だけ拾う。
/// 1字の物(行う/行なう)は送り仮名の検査(内閣告示)の担当で、ここでは扱わない
/// — 「上げる/上る」のような別語を束ねてしまう所でもある。
fn words(text: &str) -> Vec<Word> {
    let ch: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < ch.len() {
        if !is_kanji(ch[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut segs: Vec<(char, String)> = Vec::new();
        let mut surface = String::new();
        loop {
            let mut run = Vec::new();
            while i < ch.len() && is_kanji(ch[i]) {
                run.push(ch[i]);
                i += 1;
            }
            let k = okurigana(&ch, i);
            i += k.chars().count();
            for c in &run[..run.len() - 1] {
                segs.push((*c, String::new()));
                surface.push(*c);
            }
            surface.push(run[run.len() - 1]);
            surface.push_str(&k);
            segs.push((run[run.len() - 1], k.clone()));
            // 送り仮名を挟んで漢字が続くなら、まだ同じ語の中(問**い**合わせ)
            if k.is_empty() || i >= ch.len() || !is_kanji(ch[i]) {
                break;
            }
        }
        if !boundary(&ch, i) {
            continue;
        }
        if segs.len() >= 2 {
            out.push(Word { at: start, surface, segs });
        }
    }
    out
}

/// **送り仮名は前から落ちる。**
///
/// 問い合わせ → 問合せ で消えるのは「い」と「わ」で、後ろの「せ」は残る。
/// だから片方の仮名は、もう片方の**後ろ側**と一致するはず。
///
/// これが活用形どうしを落とす歯止めになる —
/// 「し」は「した」の後ろ側ではないので、引渡し と 引き渡した は束ならない。
fn drops_from_front(a: &str, b: &str) -> bool {
    a.ends_with(b) || b.ends_with(a)
}

/// 2つの書き方が「同じ語の送り仮名違い」として噛み合うか。
fn same_word(a: &Word, b: &Word) -> bool {
    a.segs.len() == b.segs.len()
        && a.segs
            .iter()
            .zip(&b.segs)
            .all(|((ka, oa), (kb, ob))| ka == kb && drops_from_front(oa, ob))
}

/// 形容詞+名詞の**句**を複合語と見誤っていないか。
///
/// 若い者/若者・青き空/青空・同じ様/同様 は、骨格キーでは束なってしまう。
/// 見分けの目印は2つ揃うこと:
///
/// 1. 違いのある送り仮名が **い・じ・き**(形容詞の連体形)で終わる
/// 2. **最後の漢字が誰も送り仮名を持たない**(=名詞で終わっている)
///
/// 問い合わせ(合→せ)や引き渡し(渡→し)は 2 に当たらないので残る。
fn adjective_glue(group: &[&Word]) -> bool {
    let n = group[0].segs.len();
    let suspect = (0..n - 1).any(|p| {
        let heads: Vec<&str> = group.iter().map(|w| w.segs[p].1.as_str()).collect();
        heads.iter().any(|o| o != &heads[0])
            && heads.iter().any(|o| o.chars().last().is_some_and(|c| ADJ_TAIL.contains(c)))
    });
    suspect && group.iter().all(|w| w.segs[n - 1].1.is_empty())
}

/// 混在1件。**どちらが正しいとは言わない。**
#[derive(Debug, Clone, PartialEq)]
pub struct Mixture {
    /// 送り仮名を落とした漢字の骨
    pub skeleton: String,
    /// 本文に現れた書き方(初出の順)と、その初出位置
    pub forms: Vec<(String, usize)>,
}

/// 文書の中で表記が混ざっている語を探す。**辞書もモデルも要らない。**
pub fn mixtures(text: &str) -> Vec<Mixture> {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<String, Vec<Word>> = BTreeMap::new();
    for w in words(text) {
        by.entry(w.skeleton()).or_default().push(w);
    }

    let mut out = Vec::new();
    for (skeleton, ws) in by {
        // 同じ書き方は初出だけ残す
        let mut forms: Vec<&Word> = Vec::new();
        for w in &ws {
            if !forms.iter().any(|f| f.surface == w.surface) {
                forms.push(w);
            }
        }
        if forms.len() < 2 {
            continue;
        }
        // 全員が互いに「送り仮名の落とし合い」になっているか
        if !forms.iter().enumerate().all(|(i, a)| forms[i + 1..].iter().all(|b| same_word(a, b))) {
            continue;
        }
        if adjective_glue(&forms) {
            continue;
        }
        // 送り仮名の無い形が音読みの熟語なら、和語との衝突(置換/置き換え)
        if forms.iter().any(|w| KANGO.contains(&w.surface.as_str())) {
            continue;
        }
        // 複合動詞の目印(内側の送り仮名)か、全員が語尾を持つこと。
        // これが無いと「確認」と「確認し」のようなサ変名詞の活用を拾う
        let interior = forms.iter().any(|w| w.has_interior());
        let tails = forms.iter().all(|w| !w.segs[w.segs.len() - 1].1.is_empty());
        if !interior && !tails {
            continue;
        }
        out.push(Mixture {
            skeleton,
            forms: forms.iter().map(|w| (w.surface.clone(), w.at)).collect(),
        });
    }
    out.sort_by_key(|m| m.forms.iter().map(|(_, at)| *at).min().unwrap_or(0));
    out
}

/// 表記ゆれの指摘。**モデルが居なくても出る。**
///
/// 1つの混在につき**書き方の数だけ**指摘を出す。どれも `candidates` は
/// 「同じ文書の中の別の書き方」で、直す先ではない —
/// 対称に並べることで「どちらが正しい」と言っていないことが目に見える。
pub fn findings(text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for m in mixtures(text) {
        for (surface, at) in &m.forms {
            out.push(Finding {
                kind: Kind::Notation,
                source: Source::Dictionary,
                found: surface.clone(),
                at: Some(*at),
                candidates: m
                    .forms
                    .iter()
                    .filter(|(s, _)| s != surface)
                    .map(|(s, _)| s.clone())
                    .collect(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mixed(text: &str) -> Vec<String> {
        mixtures(text)
            .iter()
            .map(|m| m.forms.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("/"))
            .collect()
    }

    #[test]
    fn 送り仮名の揺れを束ねる() {
        // SEKKEI が名指しした3つ
        assert_eq!(mixed("お問合せは下記。問い合わせを受け付ける。"), ["問合せ/問い合わせ"]);
        assert_eq!(mixed("引渡しの日。引き渡しを行う。"), ["引渡し/引き渡し"]);
        assert_eq!(mixed("申込は書面で。申し込みは受けない。"), ["申込/申し込み"]);
    }

    #[test]
    fn 三通り以上でも一つの束になる() {
        let m = mixtures("問合せと問い合わせと問合わせ。");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].skeleton, "問合");
        assert_eq!(m[0].forms.len(), 3, "{:?}", m[0].forms);
    }

    #[test]
    fn 揃っていれば何も言わない() {
        assert!(mixed("問い合わせは下記へ。問い合わせを受け付ける。").is_empty());
        assert!(mixed("これはただの文章です。").is_empty());
        assert!(mixed("").is_empty());
    }

    #[test]
    fn 事務の文書でよく揺れる語() {
        assert_eq!(mixed("受取りと受け取り。"), ["受取り/受け取り"]);
        assert_eq!(mixed("見積りと見積もり。"), ["見積り/見積もり"]);
        assert_eq!(mixed("打合せの後、打ち合わせを続ける。"), ["打合せ/打ち合わせ"]);
        assert_eq!(mixed("取扱を確認。取り扱いに注意。"), ["取扱/取り扱い"]);
        assert_eq!(mixed("切替の手順。切り替えを行う。"), ["切替/切り替え"]);
        assert_eq!(mixed("入口から入り口へ。"), ["入口/入り口"]);
    }

    #[test]
    fn 活用形は表記ゆれではない() {
        // 送り仮名は前から落ちる。「し」は「した」の後ろ側ではない
        assert!(!drops_from_front("し", "した"));
        assert!(drops_from_front("せ", "わせ"));
        assert!(drops_from_front("", "い"));
        // 引渡し と 引き渡した を束ねてはいけない
        assert!(mixed("引渡しの日に、書類を引き渡した。").is_empty());
    }

    #[test]
    fn 活用の途中で切らない() {
        // 「取り込む」を「取り込」として拾うと「取り込み」と混ざって見える
        assert!(mixed("取り込みを行う。データを取り込む。").is_empty());
        assert!(mixed("書き出しの処理。ファイルを書き出す。").is_empty());
        // 助詞の顔をした活用語尾(召し上**がる**)
        assert!(mixed("召し上りの品。どうぞ召し上がる。").is_empty());
    }

    #[test]
    fn サ変名詞の活用を拾わない() {
        // 「発売」と「発売し」は表記ゆれではない
        assert!(mixed("新製品発売のお知らせ。新製品を発売しても構わない。").is_empty());
        assert!(mixed("確認書類。内容を確認しても良い。").is_empty());
    }

    #[test]
    fn 形容詞と名詞の句を複合語と見誤らない() {
        // 骨格キーだけだと 若者 と 若い者 が束なる(青空文庫で実際に出た)
        assert!(mixed("若者が集う。若い者が集う。").is_empty());
        assert!(mixed("青空の下。青き空の下。").is_empty());
        assert!(mixed("同様の書式。同じ書式を使う。").is_empty());
        assert!(mixed("小村に住む。小さい村に住む。").is_empty());
        // 巻き添えにしていないこと
        assert_eq!(mixed("問合せと問い合わせ。"), ["問合せ/問い合わせ"]);
    }

    #[test]
    fn 助詞で語を繋げない() {
        // 「新製品を発売」を1語にすると「新製品発売」と混ざる
        assert!(mixed("新製品発売のお知らせ。新製品を発売。").is_empty());
        assert!(mixed("折れ線の図。折れ線も出せる。").is_empty());
    }

    #[test]
    fn 漢字一字は扱わない() {
        // 「上げる/上る」のような別語を束ねてしまう。送り仮名の検査(内閣告示)の担当
        assert!(mixed("売上が上がる。山に上る。").is_empty());
        assert!(mixed("行う。行なう。").is_empty());
    }

    #[test]
    fn 読みの違う漢語と和語を束ねない() {
        // 置換(ちかん)と 置き換え(おきかえ)は別の語。骨格が同じなだけ
        assert!(mixed("文字列の置換。値を置き換える処理。").is_empty());
        assert!(mixed("独身の男。独り身の暮らし。").is_empty());
        assert!(mixed("生物の授業。生き物を飼う。").is_empty());
    }

    #[test]
    fn 和語の熟語は漢語の一覧に載せない() {
        // 構造は 置換/置き換え と同じだが、こちらは**本物の表記ゆれ**。
        // 一覧に紛れ込ませると出なくなる
        assert_eq!(mixed("切替の手順。切り替えを行う。"), ["切替/切り替え"]);
        assert_eq!(mixed("取扱を確認。取り扱いに注意。"), ["取扱/取り扱い"]);
        assert_eq!(mixed("申込は書面。申し込みは不可。"), ["申込/申し込み"]);
        assert_eq!(mixed("引渡の日。引き渡しを行う。"), ["引渡/引き渡し"]);
        for w in KANGO {
            for bad in ["切替", "取扱", "申込", "引渡", "受付", "打合"] {
                assert_ne!(w, &bad, "和語が漢語の一覧に入っている: {bad}");
            }
        }
    }

    #[test]
    fn 位置は文字で数える() {
        // バイトで数えると日本語でずれる
        let m = mixtures("あ、問合せ。問い合わせ。");
        assert_eq!(m[0].forms[0], ("問合せ".to_string(), 2));
        assert_eq!(m[0].forms[1], ("問い合わせ".to_string(), 6));
    }

    #[test]
    fn 指摘は書き方の数だけ出て互いを指す() {
        // **どちらが正しいとは言わない。** 対称に並べる(決めごと6)
        let f = findings("問合せの窓口。問い合わせを受ける。");
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0].found, "問合せ");
        assert_eq!(f[0].candidates, vec!["問い合わせ".to_string()]);
        assert_eq!(f[1].found, "問い合わせ");
        assert_eq!(f[1].candidates, vec!["問合せ".to_string()]);
    }

    #[test]
    fn 指摘は辞書の側から出る() {
        // 辞書と規則だけで出た = GPU 無しで再現できる
        let f = findings("問合せと問い合わせ。");
        assert!(f.iter().all(|x| x.source == Source::Dictionary), "{f:?}");
        assert!(f.iter().all(|x| x.kind == Kind::Notation), "{f:?}");
    }

    #[test]
    fn 指摘の文字列は本文にそのまま在る() {
        // モデルの作り話を捨てるのと同じ掟。辞書側も守る
        let text = "引渡しの期日と、引き渡しの場所。";
        for f in findings(text) {
            assert!(text.contains(&f.found), "本文に無い: {}", f.found);
        }
    }

    #[test]
    fn 壊れた入力でも落ちない() {
        for s in ["", "、", "問", "問い", "い問", "々", "漢字漢字", "\u{3005}\u{3005}"] {
            let _ = findings(s);
        }
    }
}
