//! 生成物 `sheet/src/datetime_names.rs` の見張り。
//!
//! **生成ファイルの中に試験を書かない。** あちらの頭には「手で書かない」と
//! あり、作り直せば消える(2026-08-10、一度そこに書いて気づいた)。
//! 生成物と、それを見る目は別のファイルに置く。

use sheet::datetime_names::{names, TABLE};



/// 生成物が痩せていないこと。**空の語が1つでもあると、その月だけ
/// 何も出ない日付ができる** — 生成のときも見ているが、手で触られたら
/// ここで落とす
#[test]
fn every_language_is_complete() {
    // **数は書かない。** 言語が増えるたびに試験の名前と数を追いかけることに
    // なり、追いかけ損ねた瞬間に名前のほうが嘘になる(2026-08-11、pt-PT を
    // 足して気づいた)。ここで要るのは「欠けが無いこと」
    assert!(TABLE.len() >= 14, "言語が減っている({} 件)", TABLE.len());
    assert!(TABLE.iter().any(|n| n.lang == "ja"), "素の言語が無い");
    for n in TABLE {
        for (what, arr) in [("months", &n.months[..]), ("months_abbr", &n.months_abbr[..])] {
            assert_eq!(arr.len(), 12, "{}: {what}", n.lang);
            assert!(arr.iter().all(|s| !s.is_empty()), "{}: {what} に空がある", n.lang);
        }
        for (what, arr) in [("days", &n.days[..]), ("days_abbr", &n.days_abbr[..])] {
            assert_eq!(arr.len(), 7, "{}: {what}", n.lang);
            assert!(arr.iter().all(|s| !s.is_empty()), "{}: {what} に空がある", n.lang);
        }
        assert!(!n.long_date.is_empty(), "{}: 長い日付の既定が無い", n.lang);
    }
}

/// **書式コードにバックスラッシュを残さない。** こちらの字句走査は
/// `\` を逃げとして扱わないので、残っていると画面に `\` が出る
#[test]
fn no_fallback_left_in_the_default_format() {
    for n in TABLE {
        assert!(
            !n.long_date.contains('\\'),
            "{}: 逃げが残っている {:?}",
            n.lang,
            n.long_date
        );
    }
}

/// 引けること。**知らない言語は日本語**(素の言語だから) —
/// 黙って英語にすると、日本語で使っている人に英語が出る
#[test]
fn can_look_up_a_language() {
    assert_eq!(names("de").months[7], "August");
    assert_eq!(names("ja").months[7], "8月");
    assert_eq!(names("ja").days[1], "月曜日");
    // 枝つきは枝ごと、無ければ根へ
    assert_eq!(names("zh-tw").months[7], "八月");
    assert_eq!(names("zh-Hant").lang, "zh", "枝が無ければ根に落ちる");
    // 知らない言語は日本語
    assert_eq!(names("xx").lang, "ja");
    assert_eq!(names("").lang, "ja");
}

/// 属格を持つ言語は持ち、持たない言語は None。**露語で確かめる** —
/// 「8月」と「8月の」で形が違うので、混ぜると日付が不自然になる
#[test]
fn genitive_forms_are_kept() {
    let ru = names("ru");
    let g = ru.months_genitive.expect("露語は属格を持つ");
    assert_eq!(ru.months[7], "Август", "主格");
    assert_eq!(g[7], "августа", "属格");
    assert!(names("de").months_genitive.is_none(), "独語は属格を持たない");
}

/// 台湾に香港の物を渡さない。**本家の zh-Hant は香港**なので、
/// 素直に引くと通貨も月名も香港のものになる(通貨はここに載せていないが、
/// 引き当ての癖は同じ)
#[test]
fn taiwan_and_hong_kong_are_not_mixed_up() {
    // zh-TW の曜日。香港と同じ字だが、材料は zh-TW から取っている
    assert_eq!(names("zh-tw").days[1], "星期一");
    assert_eq!(names("zh-tw").long_date, "yyyy\"年\"m\"月\"d\"日\"");
}
