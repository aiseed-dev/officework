//! 条件付き書式が **xlsx を往復しても同じ見えになる**ことの検査。
//!
//! [`kumihan::look::resolve_cond`] は 2026-08-14 に新設した「表のセルの見えを
//! 決める1本」で、画面(calc/src/view.rs)と紙(paper/src/grid.rs)の
//! **両方がこれを通る**。それまでは当てはめが2箇所に写して書かれていて、
//! 画面はバーもスケールもアイコンも描くのに紙は塗りと文字色だけ、という
//! 食い違いが育っていた。
//!
//! ここが見るのは「読んだ規則が正しく当てはまるか」— `look.rs` の中の
//! 単体試験(規則の重ね方)と合わせて、読み→当てはめの一本道を縛る。

use kumihan::look::resolve_cond;
use kumihan::book::{CondKind, CondLook, CondOp, CondRule};
use kumihan::book::{Book, Cell, Pos, Value};
use sheet::xlsx;

fn round(book: &Book) -> Book {
    let mut buf = std::io::Cursor::new(Vec::new());
    xlsx::write(book, &mut buf).expect("書けない");
    buf.set_position(0);
    xlsx::read(buf).expect("読めない").0
}

/// A〜D の4列に 10/30/50/70/90 を入れ、列ごとに違う種類の規則を掛ける
fn book_with_rules() -> Book {
    let mut book = Book::new();
    let s = &mut book.sheets[0];
    for (i, v) in [10, 30, 50, 70, 90].iter().enumerate() {
        let r = i as u32;
        for c in 0..4 {
            s.set(Pos::new(r, c), Cell::input(&v.to_string()));
        }
    }
    let range = |c: u32| (Pos::new(0, c), Pos::new(4, c));
    s.cond = vec![
        CondRule {
            range: range(0),
            kind: CondKind::Cmp(CondOp::Gt, 50.0),
            look: CondLook {
                fill: Some("FFC7CE".into()),
                color: Some("9C0006".into()),
                bold: Some(true),
                ..Default::default()
            },
        },
        CondRule { range: range(1), kind: CondKind::Bar("638EC6".into()), look: CondLook::default() },
        CondRule {
            range: range(2),
            kind: CondKind::Scale("F8696B".into(), None, "63BE7B".into()),
            look: CondLook::default(),
        },
        CondRule { range: range(3), kind: CondKind::Icons("3Arrows".into()), look: CondLook::default() },
    ];
    book
}

#[test]
fn four_rule_kinds_round_trip_to_the_same_look() {
    for (name, book) in [("元", book_with_rules()), ("往復後", round(&book_with_rules()))] {
        let s = &book.sheets[0];
        let prep: Vec<_> = s.cond.iter().map(|r| (r.clone(), r.aux(s))).collect();
        let look = |r: u32, c: u32| resolve_cond(&prep, Pos::new(r, c), &s.value(Pos::new(r, c)));

        // A列: 50 より大きいところだけ塗り・文字色・太字が付く
        assert_eq!(look(2, 0).fill, None, "{name}: 50 は「より大きい」に当たってはいけない");
        let hit = look(3, 0);
        assert_eq!(hit.fill.as_deref(), Some("FFC7CE"), "{name}: 塗り");
        assert_eq!(hit.color.as_deref(), Some("9C0006"), "{name}: 文字色");
        assert_eq!(hit.bold, Some(true), "{name}: 太字");

        // B列: データバーは 10→0.0、90→1.0
        let (t0, c0) = look(0, 1).bar.clone().unwrap_or_else(|| panic!("{name}: 棒が無い"));
        assert_eq!((t0, c0.as_str()), (0.0, "638EC6"), "{name}: 最小値の棒");
        assert_eq!(look(4, 1).bar.unwrap().0, 1.0, "{name}: 最大値の棒");

        // C列: カラースケールは**塗りとして**返る(紙もこれを塗る)
        assert!(look(0, 2).fill.is_some(), "{name}: スケールの色が塗りに出ない");
        assert!(look(0, 2).bar.is_none(), "{name}: スケールが棒になっている");

        // D列: アイコンは3段
        assert_eq!(look(0, 3).icon.unwrap().0, "↓", "{name}: 下段");
        assert_eq!(look(4, 3).icon.unwrap().0, "↑", "{name}: 上段");
    }
}

#[test]
fn columns_without_rules_stay_plain() {
    let book = book_with_rules();
    let s = &book.sheets[0];
    let prep: Vec<_> = s.cond.iter().map(|r| (r.clone(), r.aux(s))).collect();
    // E列には規則が無い
    let r = resolve_cond(&prep, Pos::new(0, 4), &Value::Number(90.0));
    assert_eq!(r, kumihan::look::CondResolved::default());
}
