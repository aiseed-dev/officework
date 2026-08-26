//! 校正の入口 — **1つの窓口で、言語ごとに正しい道具へ振り分ける。**
//!
//! hunspell が英語にしたことを日本語でやる、というのがこのソフトの存在理由。
//! だから校正は「うちのワープロの機能」ではなく、**単体で使える道具**として作る
//! (`office-spell` コマンド)。誰でも同梱できて、金を払わず、手元で動く。
//!
//! 振り分け:
//!
//! | 本文 | 誤りの正体 | 道具 | GPU |
//! |---|---|---|---|
//! | 英語 | 綴り誤りは**辞書に無い語**になる (`recieve`) | 辞書 | 要らない |
//! | 日本語 | 誤変換は**辞書に有る語**になる (以外/意外) | モデル | 要る |
//!
//! 混在した文書は**両方**を掛ける(日本語の文書に英単語が混ざるのは普通)。
//!
//! **一番やってはいけないのは、動かなかったときに黙って「指摘なし」と出すこと。**
//! 利用者は「誤りが無い」と受け取る。だから何を検査できて何を検査できなかったかを
//! 必ず持ち帰る(`Report::skipped`)。

use crate::ja::furigana::{self, Suggestion};
use crate::Target;
use crate::model::Endpoint;
use crate::ja::{homophone, notation, okurigana, proof, wording};
use crate::spell::{self, Dictionary};

/// 指摘の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 英語の綴り誤り
    Spelling,
    /// 辞書に無いが、固有名詞か誤りか判定できなかった語
    UnknownWord,
    /// 誤変換 — 辞書に有る語なので辞書では捕まらない
    Conversion,
    /// 表記ゆれ
    Notation,
    /// 送り仮名
    Okurigana,
    /// 重複表現・助詞など
    Wording,
    /// ふりがな(誤りではなく、付ける候補)
    Reading,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Spelling => "綴り",
            Kind::UnknownWord => "未知語",
            Kind::Conversion => "誤変換",
            Kind::Notation => "表記ゆれ",
            Kind::Okurigana => "送り仮名",
            Kind::Wording => "言い回し",
            Kind::Reading => "ふりがな",
        }
    }

    fn from_why(why: &str) -> Kind {
        if why.contains("誤変換") {
            Kind::Conversion
        } else if why.contains("表記") {
            Kind::Notation
        } else if why.contains("送り") {
            Kind::Okurigana
        } else {
            Kind::Wording
        }
    }
}

/// どちらの道具が出したか。**辞書で出たものは GPU 無しで再現できる。**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Dictionary,
    Model,
}

/// 指摘1件。**`candidates` は順序が意味を持つ。断定はしない。**
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub kind: Kind,
    pub source: Source,
    /// 本文中の該当文字列
    pub found: String,
    /// 本文における文字位置(分からなければ None)
    pub at: Option<usize>,
    /// 直し方・読み方の候補(高い順)。
    /// **表記ゆれだけは「直す先」ではなく「同じ文書の中の別の書き方」** —
    /// どちらが正しいとは言わない(SEKKEI 決めごと6)
    pub candidates: Vec<String>,
}

/// 検査の結果。**何を検査できなかったかを必ず持つ。**
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// 検査できなかったもの(理由つき)。空でなければ「指摘なし」と言ってはいけない
    pub skipped: Vec<String>,
}

impl Report {
    /// 「指摘はありません」と言ってよいか。
    ///
    /// **検査できなかった部分があるときは言ってはいけない。**
    pub fn may_say_clean(&self) -> bool {
        self.findings.is_empty() && self.skipped.is_empty()
    }

    /// 画面や端末に出す一行。
    pub fn summary(&self) -> String {
        match (self.findings.len(), self.skipped.len()) {
            (0, 0) => "指摘はありません".into(),
            (n, 0) => format!("{n} 件の指摘"),
            (0, _) => format!("検査できませんでした — {}", self.skipped.join(" / ")),
            (n, _) => format!("{n} 件の指摘(ただし {})", self.skipped.join(" / ")),
        }
    }
}

/// 校正の道具立て。辞書は起動時に1回だけ読む。
pub struct Checker {
    pub dict: Option<Dictionary>,
    pub dict_problem: Option<String>,
    pub endpoint: Endpoint,
    /// 同音異義語を含む文だけをモデルへ送るか。**既定は false。**
    ///
    /// 落とせるのは「大丈夫と証明できた文」ではなく「表に載っていない文」
    /// なので、既定にはしない([`crate::ja::homophone`])。
    /// 明示で頼まれたときだけ濾過し、見ていない文の数を `skipped` に残す
    pub filter_homophones: bool,
    /// 一度判定した語は覚える。**同じ語を二度モデルに訊かない**
    seen: std::cell::RefCell<std::collections::BTreeMap<String, Verdict>>,
}

impl Default for Checker {
    fn default() -> Self {
        let (dict, dict_problem) = match Dictionary::load_default() {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            dict,
            dict_problem,
            endpoint: Endpoint::default(),
            filter_homophones: false,
            seen: Default::default(),
        }
    }
}

impl Checker {
    /// 本文を検査する。**言語で道具を振り分け、混在文には両方を掛ける。**
    pub fn check(&self, text: &str) -> Report {
        let mut r = Report::default();
        if text.trim().is_empty() {
            return r;
        }
        let lang = spell::lang_of(text);

        // --- 英語 ---
        //
        // **辞書は答えではなく絞り込み。** 辞書に無い語が誤りとは限らない
        // (`Bennet` `Bingley` `Radeon` は正しい)。実測では
        //   『高慢と偏見』 123,688語 → 辞書に無い語 312 (0.25%)
        //   技術文書        2,114語 → 辞書に無い語  74 (3.50%)
        // つまり**辞書が99.7%を落としてから、残りだけモデルに訊けばよい**。
        // 一番速い推論は、動かさない推論。
        match (&self.dict, &self.dict_problem) {
            (Some(d), _) => {
                let unknown = d.check(text);
                let verdicts = self.classify_unknown(text, &unknown, &mut r);
                for m in unknown {
                    match verdicts.get(m.word.as_str()) {
                        // 固有名詞と判定された。誤りではない
                        Some(Verdict::Name) => {}
                        Some(Verdict::Typo) => r.findings.push(Finding {
                            kind: Kind::Spelling,
                            source: Source::Model,
                            found: m.word,
                            at: Some(m.at),
                            candidates: m.suggestions,
                        }),
                        // 判定できなかった。**誤りと断定しない**
                        None => r.findings.push(Finding {
                            kind: Kind::UnknownWord,
                            source: Source::Dictionary,
                            found: m.word,
                            at: Some(m.at),
                            candidates: m.suggestions,
                        }),
                    }
                }
            }
            // 辞書が無いのに黙って通さない
            (None, Some(e)) => r.skipped.push(format!("英語の綴り({e})")),
            (None, None) => {}
        }

        // --- 日本語 ---
        if lang == spell::Lang::Japanese {
            // 表記ゆれは**文書の中だけで判る**ので、モデルが居なくても出す。
            // GPU の無い機械で日本語の校正が動き出すのはここ
            r.findings.extend(notation::findings(text));
            // 重複表現も有限の一覧で閉じる。**誤りとは言わず、言い換えの案を出す**
            r.findings.extend(wording::findings(text));
            // 送り仮名は内閣告示による。**許容を誤りにしない**(決めごと4)
            r.findings.extend(okurigana::findings(text));

            // 誤変換・重複表現はモデル。辞書では原理的に捕まらない。
            // **繋がらなければそう言う** — 表記ゆれが出たからといって
            // 「検査できなかった部分がある」(終了コード3)は消えない
            // ここまでが辞書の指摘。モデルの分と突き合わせて重複を落とす
            let by_dict = r.findings.len();

            // 濾過を頼まれていれば、紛らわしい語のある文だけを渡す。
            // **落とした文は「大丈夫」ではなく「見ていない」** — 必ずそう言う
            let cut = self.filter_homophones.then(|| homophone::filter(text));
            let to_model = match &cut {
                Some(f) => f.send.as_str(),
                None => text,
            };
            if let Some(f) = &cut {
                if !f.is_whole() {
                    r.skipped.push(format!(
                        "同音異義語の無い {} 文(全 {} 文)はモデルに渡していない",
                        f.dropped, f.total
                    ));
                }
            }

            match proof::proofread(&self.endpoint, to_model) {
                Ok(notes) => {
                    for n in notes {
                        let at = char_index_of(text, &n.found);
                        let f = Finding {
                            kind: Kind::from_why(&n.why),
                            source: Source::Model,
                            found: n.found,
                            at,
                            candidates: vec![n.suggest],
                        };
                        // **同じ語を2回言わない。**
                        // モデルへの指示は狭めない — 辞書が取りこぼす形
                        // (問合せ先/問い合わせ先)はモデルにしか見えないから
                        if !already_said(&r.findings[..by_dict], &f) {
                            r.findings.push(f);
                        }
                    }
                }
                Err(e) => r.skipped.push(format!("日本語の校正({e})")),
            }
        }

        r.findings.sort_by_key(|f| f.at.unwrap_or(usize::MAX));
        r
    }

    /// ふりがなの候補を出す。**誤りの指摘ではなく、付ける候補。**
    pub fn readings(&self, text: &str, targets: &[Target]) -> Result<Vec<Suggestion>, String> {
        furigana::candidates(&self.endpoint, text, targets).map(|(s, _)| s)
    }
}

/// 辞書に無い語の正体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 固有名詞・商標・地名など。誤りではない
    Name,
    /// 綴り誤り
    Typo,
}

const NAME_SYSTEM: &str = "英語の文章から、辞書に載っていない語を抜き出しました。\
それぞれが「固有名詞(人名・地名・会社名・製品名・商標・専門用語)」なのか\
「綴り誤り」なのかを、本文を読んで判定してください。\
迷ったら name にしてください(正しい語を誤りだと言う方が害が大きい)。\
出力は JSON の配列のみ。各要素は {\"word\":\"語\",\"verdict\":\"name\" または \"typo\"}。\
渡された語だけを判定し、語を増やさないでください。";

impl Checker {
    /// 辞書に無い語が、固有名詞か誤りかをモデルに訊く。
    ///
    /// **これは「AI機能」ではない。** 辞書が原理的に答えられない問いを、
    /// 答えられる道具に回しているだけ。
    /// モデルが居なければ空を返す — 呼び出し側は**誤りと断定しない**。
    fn classify_unknown(
        &self,
        text: &str,
        unknown: &[spell::Misspelling],
        report: &mut Report,
    ) -> std::collections::BTreeMap<String, Verdict> {
        use std::collections::BTreeMap;
        let mut out: BTreeMap<String, Verdict> = BTreeMap::new();
        if unknown.is_empty() {
            return out;
        }
        // 一度判定した語は覚えている(同じ文書で何度も訊かない)
        let mut ask: Vec<&str> = Vec::new();
        {
            let seen = self.seen.borrow();
            for m in unknown {
                if let Some(v) = seen.get(&m.word) {
                    out.insert(m.word.clone(), *v);
                } else if !ask.contains(&m.word.as_str()) {
                    ask.push(&m.word);
                }
            }
        }
        if ask.is_empty() {
            return out;
        }
        let user = format!("本文:\n{}\n\n判定する語: {}", snippet(text, 4000), ask.join(", "));
        match crate::model::chat(&self.endpoint, NAME_SYSTEM, &user, 0.0) {
            Ok(reply) => {
                let mut seen = self.seen.borrow_mut();
                for obj in crate::model::objects(&reply.content) {
                    let (Some(w), Some(v)) =
                        (crate::model::field(obj, "word"), crate::model::field(obj, "verdict"))
                    else {
                        continue;
                    };
                    // 訊いていない語は通さない
                    if !ask.contains(&w.as_str()) {
                        continue;
                    }
                    let verdict = match v.trim().to_lowercase().as_str() {
                        "name" => Verdict::Name,
                        "typo" => Verdict::Typo,
                        _ => continue,
                    };
                    seen.insert(w.clone(), verdict);
                    out.insert(w, verdict);
                }
            }
            Err(e) => {
                // 判定できないことを黙らない。呼び出し側は「未知語」として出す
                report.skipped.push(format!("固有名詞の判定({e})"));
            }
        }
        out
    }
}

/// モデルの指摘を、辞書が既に言っているか。
///
/// 1〜3 が入って辞書とモデルの守備範囲が重なった。利用者から見れば指摘は1つで、
/// **どちらが言ったかは関係ない**(決めごと1)。残すのは**辞書の側** —
/// GPU の有無で出る物が変わらないほうがよい。
///
/// 種別が同じで、文字列が一方に含まれていれば同じ指摘と見る
/// (モデルは「お問合せ」と広く取ることがある)。
fn already_said(by_dict: &[Finding], note: &Finding) -> bool {
    by_dict.iter().any(|d| {
        d.kind == note.kind
            && (d.found.contains(&note.found) || note.found.contains(&d.found))
    })
}

/// 長すぎる本文は頭を渡す(判定には全文は要らない)。
fn snippet(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// 本文の中の文字位置(バイトではなく文字)。
fn char_index_of(text: &str, needle: &str) -> Option<usize> {
    let b = text.find(needle)?;
    Some(text[..b].chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker_with_dict() -> Checker {
        Checker {
            dict: Some(Dictionary::from_list("the\nreceive\ndocument\nis\nhere\n")),
            dict_problem: None,
            // 繋がらない宛先。モデル側は必ず失敗する
            endpoint: Endpoint { port: 1, ..Default::default() },
            filter_homophones: false,
            seen: Default::default(),
        }
    }

    #[test]
    fn dict_narrows_candidates() {
        // 辞書に無い語を見つけるところまでは、モデル無しで動く
        let c = checker_with_dict();
        let r = c.check("the documnt is here");
        assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
        assert_eq!(r.findings[0].found, "documnt");
        assert!(r.findings[0].candidates.contains(&"document".to_string()));
    }

    #[test]
    fn no_spelling_verdict_when_undecidable() {
        // 辞書に無い語が誤りとは限らない(Bennet・Radeon は正しい)。
        // モデルが居ないなら「未知語」までしか言わない
        let c = checker_with_dict();
        let r = c.check("the documnt is here");
        assert_eq!(r.findings[0].kind, Kind::UnknownWord, "誤りと断定してしまった");
        assert!(
            r.skipped.iter().any(|s| s.contains("固有名詞")),
            "判定できなかったことを黙っている: {:?}",
            r.skipped
        );
    }

    #[test]
    fn proper_noun_not_flagged() {
        let c = checker_with_dict();
        c.seen.borrow_mut().insert("Bennet".into(), Verdict::Name);
        c.seen.borrow_mut().insert("documnt".into(), Verdict::Typo);
        let r = c.check("Bennet wrote the documnt");
        let words: Vec<&str> = r.findings.iter().map(|f| f.found.as_str()).collect();
        assert!(!words.contains(&"Bennet"), "固有名詞を誤りにした: {words:?}");
        assert!(words.contains(&"documnt"), "本物の誤りを落とした: {words:?}");
        let t = r.findings.iter().find(|f| f.found == "documnt").unwrap();
        assert_eq!(t.kind, Kind::Spelling);
        assert_eq!(t.source, Source::Model, "判定はモデルの仕事");
    }

    #[test]
    fn learned_words_not_asked_twice() {
        // 判定はモデルを動かす。同じ語で何度も動かさない
        let c = checker_with_dict();
        c.seen.borrow_mut().insert("Bennet".into(), Verdict::Name);
        let r = c.check("Bennet is here Bennet is here");
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        assert!(
            r.skipped.is_empty(),
            "全部覚えていれば問い合わせは要らないはず: {:?}",
            r.skipped
        );
    }

    #[test]
    fn japanese_needs_model() {
        let c = checker_with_dict();
        let r = c.check("それは以外な結果でした。");
        // 繋がらないので検査できない。**黙って「指摘なし」にしない**
        assert!(!r.skipped.is_empty(), "モデルが無いのに検査できたことにした");
        assert!(!r.may_say_clean(), "指摘なしと言ってはいけない場面で言えてしまう");
        assert!(r.summary().contains("検査できません"), "{}", r.summary());
    }

    #[test]
    fn variants_found_without_model() {
        // **これが辞書側を作った理由。** GPU の無い機械でも日本語の指摘が出る
        let c = checker_with_dict();
        let r = c.check("お問合せは下記まで。問い合わせを受け付けます。");
        let n: Vec<&Finding> = r.findings.iter().filter(|f| f.kind == Kind::Notation).collect();
        assert_eq!(n.len(), 2, "{:?}", r.findings);
        assert!(n.iter().all(|f| f.source == Source::Dictionary), "{n:?}");
        // どちらが正しいとは言わない。互いを指すだけ
        assert_eq!(n[0].found, "問合せ");
        assert_eq!(n[0].candidates, vec!["問い合わせ".to_string()]);
    }

    #[test]
    fn variants_do_not_clear_unchecked() {
        // 終了コード3(検査できなかった部分がある)は辞書が通っただけでは消えない。
        // **黙って「指摘なし」にしない**の裏返し — 黙って「全部見た」にもしない
        let c = checker_with_dict();
        let r = c.check("お問合せは下記まで。問い合わせを受け付けます。");
        assert!(!r.findings.is_empty(), "表記ゆれが出ていない");
        assert!(
            r.skipped.iter().any(|s| s.contains("日本語")),
            "モデルに訊けなかったことを黙っている: {:?}",
            r.skipped
        );
        assert!(!r.may_say_clean());
        assert!(r.summary().contains("ただし"), "{}", r.summary());
    }

    #[test]
    fn consistent_japanese_cannot_claim_none() {
        // 表記ゆれが無くても、誤変換はモデルにしか見えない。
        // 辞書が通ったからといって「綺麗です」と言ってはいけない
        let c = checker_with_dict();
        let r = c.check("問い合わせを受け付けます。");
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        assert!(!r.may_say_clean(), "モデル抜きで「指摘なし」と言えてしまう");
    }

    #[test]
    fn model_does_not_repeat_dict() {
        // 1〜3 で辞書とモデルの守備範囲が重なった。**同じ語を2回出さない**
        let dict = vec![Finding {
            kind: Kind::Notation,
            source: Source::Dictionary,
            found: "問合せ".into(),
            at: Some(1),
            candidates: vec!["問い合わせ".into()],
        }];
        let same = Finding {
            kind: Kind::Notation,
            source: Source::Model,
            found: "問合せ".into(),
            at: Some(1),
            candidates: vec!["問い合わせ".into()],
        };
        assert!(already_said(&dict, &same), "同じ指摘を落とせていない");

        // モデルが広く取った場合も同じ指摘
        let wider = Finding { found: "お問合せ".into(), ..same.clone() };
        assert!(already_said(&dict, &wider), "広く取った同じ指摘を落とせていない");

        // 種別が違えば別の指摘。誤変換は辞書には見えない
        let other = Finding { kind: Kind::Conversion, found: "以外".into(), ..same.clone() };
        assert!(!already_said(&dict, &other), "別の指摘まで落とした");

        // 辞書が触れていない語はモデルの指摘が残る
        let elsewhere = Finding { found: "打合せ".into(), ..same.clone() };
        assert!(!already_said(&dict, &elsewhere), "無関係な指摘まで落とした");
    }

    #[test]
    fn filtering_off_by_default() {
        // 落とせるのは「大丈夫と証明できた文」ではなく「表に載っていない文」。
        // **既定にはしない**
        let c = checker_with_dict();
        assert!(!c.filter_homophones, "濾過が既定で効いている");
        let r = c.check("犬が走る。猫が寝る。");
        assert!(
            !r.skipped.iter().any(|s| s.contains("渡していない")),
            "既定なのに文を落とした: {:?}",
            r.skipped
        );
    }

    #[test]
    fn filtering_reports_dropped() {
        // **黙って「指摘なし」にしない**の裏返し。見ていない範囲は必ず言う
        let mut c = checker_with_dict();
        c.filter_homophones = true;
        let r = c.check("犬が走る。猫が寝る。それは以外な結果でした。");
        let told = r.skipped.iter().any(|s| s.contains("渡していない") && s.contains("2 文"));
        assert!(told, "落とした文を黙っている: {:?}", r.skipped);
        assert!(!r.may_say_clean());
    }

    #[test]
    fn mixed_text_runs_both() {
        let c = checker_with_dict();
        let r = c.check("Radeon の documnt を確認する");
        // 英語側は辞書で出る
        assert!(r.findings.iter().any(|f| f.found == "documnt"), "{:?}", r.findings);
        // 日本語側は検査できなかったと残る
        assert!(r.skipped.iter().any(|s| s.contains("日本語")), "{:?}", r.skipped);
    }

    #[test]
    fn clean_english_may_report_none() {
        let c = checker_with_dict();
        let r = c.check("the document is here");
        assert!(r.may_say_clean());
        assert_eq!(r.summary(), "指摘はありません");
    }

    #[test]
    fn missing_dict_is_reported() {
        let c = Checker {
            dict: None,
            dict_problem: Some("辞書が見つかりません".into()),
            endpoint: Endpoint { port: 1, ..Default::default() },
            filter_homophones: false,
            seen: Default::default(),
        };
        let r = c.check("the documnt");
        assert!(!r.may_say_clean(), "辞書が無いのに「指摘なし」と言えてしまう");
        assert!(r.skipped.iter().any(|s| s.contains("英語")), "{:?}", r.skipped);
    }

    #[test]
    fn empty_text_does_nothing() {
        let c = checker_with_dict();
        let r = c.check("   \n  ");
        assert!(r.may_say_clean());
    }

    #[test]
    fn hits_in_text_order() {
        let c = checker_with_dict();
        let r = c.check("documnt and anothr");
        let ats: Vec<usize> = r.findings.iter().filter_map(|f| f.at).collect();
        assert!(ats.windows(2).all(|w| w[0] <= w[1]), "順に並んでいない: {ats:?}");
    }

    #[test]
    fn kind_dispatch() {
        assert_eq!(Kind::from_why("誤変換"), Kind::Conversion);
        assert_eq!(Kind::from_why("表記ゆれ"), Kind::Notation);
        assert_eq!(Kind::from_why("送り仮名"), Kind::Okurigana);
        assert_eq!(Kind::from_why("重複表現"), Kind::Wording);
        assert_eq!(Kind::from_why(""), Kind::Wording);
    }

    #[test]
    fn char_position_counted_in_chars() {
        // バイトで数えると日本語で位置がずれる
        assert_eq!(char_index_of("あいう以外です", "以外"), Some(3));
        assert_eq!(char_index_of("本文", "無い語"), None);
    }
}
