//! 往復で落ちる物の見張り(`crate::holes` の試験)。

/// **見張りの表が `types.rs` と揃っているか。**
///
/// `Sheet` に持ち物を足したとき [`crate::crate::holes::WATCHED`] に足し忘れると、
/// その持ち物は落ちても誰も何も言いません。**穴を数える道具に穴が空く**ので、
/// ここで機械に照合させます。
#[cfg(test)]
mod holes_watch {
    use crate::holes::{WATCHED, WATCHED_BOOK};

    /// `types.rs` の `pub struct <名前> { … }` から `pub` の持ち物の名前を拾う
    fn fields_of(name: &str) -> Vec<String> {
        let src = include_str!("../../book/src/types.rs");
        let head = format!("pub struct {name} {{");
        let from = src.find(&head).unwrap_or_else(|| panic!("{name} が見つからない"));
        let body = &src[from + head.len()..];
        let to = body.find("\n}").unwrap_or_else(|| panic!("{name} の終わりが無い"));
        body[..to]
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub "))
            .filter_map(|l| l.split(':').next())
            .map(|s| s.trim().to_string())
            .collect()
    }

    #[test]
    fn every_sheet_field_is_watched() {
        let mine: Vec<&str> = WATCHED.iter().map(|(n, _)| *n).collect();
        for f in fields_of("Sheet") {
            assert!(
                mine.contains(&f.as_str()),
                "Sheet の持ち物「{f}」が holes::WATCHED に無い。\
                 足すと往復で落ちても誰も言わなくなります"
            );
        }
        for m in &mine {
            assert!(
                fields_of("Sheet").iter().any(|f| f == m),
                "holes::WATCHED の「{m}」が Sheet に無い(持ち物を消したのに表が残っている)"
            );
        }
    }

    #[test]
    fn every_book_field_is_watched() {
        let mine: Vec<&str> = WATCHED_BOOK.iter().map(|(n, _)| *n).collect();
        for f in fields_of("Book") {
            assert!(mine.contains(&f.as_str()), "Book の持ち物「{f}」が WATCHED_BOOK に無い");
        }
        for m in &mine {
            assert!(fields_of("Book").iter().any(|f| f == m), "WATCHED_BOOK の「{m}」が Book に無い");
        }
    }
}

/// **`.sheet.adoc` の往復で落ちる物を数える。** 正本にする作業の物差しです。
///
/// 落ちた数は増やしてはいけません。減らすときはこの数を書き替えます。
#[cfg(test)]
mod holes_count {
    use crate::holes::round_trip_holes;

    /// **いま埋まっていない穴。** 空です — 2026-08-26 に全部埋まりました。
    ///
    /// 増えたら試験が落ちます。持ち物を足して往復させ忘れると、ここで
    /// 止まります。
    const KNOWN: &[&str] = &[];

    #[test]
    fn the_round_trip_holes_are_the_known_ones() {
        // **言語の錠を取ります。** 往復はテンプレートを通るので、他の試験が
        // 画面の言語を替えている間に走ると崩れます。台湾の中国語は「列」が
        // 行のことなので、日本語の「列」が行として読まれます
        // (2026-08-28。20 回に1回ほど落ちるのを捕まえました)
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let now = round_trip_holes();
        let extra: Vec<_> = now.iter().filter(|n| !KNOWN.contains(n)).collect();
        let fixed: Vec<_> = KNOWN.iter().filter(|n| !now.contains(n)).collect();
        assert!(
            extra.is_empty(),
            "往復で落ちる物が増えました: {extra:?}\n\n{}",
            crate::holes::report()
        );
        assert!(
            fixed.is_empty(),
            "穴が埋まりました。KNOWN から外してください: {fixed:?}"
        );
    }

    /// 作業の一覧を読むための入り口。`-- --nocapture` で中身が出ます
    #[test]
    fn shows_what_is_still_lost() {
        print!("{}", crate::holes::report());
    }
}
