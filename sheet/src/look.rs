//! **表のセルの見えを決める1本。** SEKKEI「層は6つ」の表で、
//! 「見え」の層は紙面側が `kumihan::layout`、**格子側は無い**と書いてあった
//! 穴がここ。画面(`calc/src/view.rs`)と紙(`paper/src/grid.rs`)は
//! どちらもこの関数を通し、**同じ規則で同じ答え**を得る。
//!
//! 2026-08-14 まで、条件付き書式の当てはめは画面と紙の2箇所に写して
//! 書かれていた。写しは揃わない — 実際、画面は 39 行あってデータバーも
//! カラースケールもアイコンも描くのに、紙は 11 行で塗り・文字色・太字だけ
//! だった。**同じブックが画面と紙で違って見えていた**(SEKKEI の
//! 「builder と無関係に壊れている物」がこれ)。
//!
//! ここは**何を描くかを決めるだけ**で、描きはしない。GPUI の div も
//! printpdf の Rect もここには出てこない — だから画面と紙の両方から呼べる。

use crate::model::{CondAux, CondKind, CondRule, Value};
use crate::Pos;

/// 条件付き書式が、あるセル1つについて決めたこと。
///
/// 飾りが `Option<bool>` なのは [`crate::model::CondLook`] と同じ三択だから —
/// **`None` は「触らない」**(セル自身の書式のまま)で、`Some(false)` だけが外す。
/// `bool` に潰すと、元から太字のセルが規則に当たった途端に細くなる。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CondResolved {
    /// 塗り RRGGBB。**カラースケールの色もここに入る** — 描く側から見れば
    /// どちらも「このセルの塗り」で、区別する必要が無い
    pub fill: Option<String>,
    /// 文字色 RRGGBB
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// データバー(0〜1 の長さ, 棒の色 RRGGBB)。文字の下に敷く
    pub bar: Option<(f64, String)>,
    /// アイコン(出す字, 色 RRGGBB)
    pub icon: Option<(&'static str, &'static str)>,
}

/// アイコンセットの3段の色。信号の並び(下=赤・中=黄・上=緑)。
const ICON_LOW: &str = "C62828";
const ICON_MID: &str = "E6A700";
const ICON_HIGH: &str = "2E7D32";

/// 物差しの値(0〜1)を3段のアイコンにする。
///
/// 名前に `Arrow` を含むセットは矢印、それ以外は信号の丸。**xlsx の
/// iconSet は 20 種類以上あるが、描き分けられるのはこの2系統だけ** —
/// 「並べるのは画面で描き分けられる物だけ」(柄を18種→6種に絞ったのと同じ線)。
/// 3段より細かいセット(4段・5段)も3段に丸める。**丸めたことは見れば
/// 分かる**(矢印の向きが3つしかない)ので、黙って別の物を描くよりは良い
fn icon_of(set: &str, t: f64) -> (&'static str, &'static str) {
    let arrows = set.contains("Arrow");
    if t < 1.0 / 3.0 {
        (if arrows { "↓" } else { "●" }, ICON_LOW)
    } else if t < 2.0 / 3.0 {
        (if arrows { "→" } else { "●" }, ICON_MID)
    } else {
        (if arrows { "↑" } else { "●" }, ICON_HIGH)
    }
}

/// このセルに条件付き書式が何をするかを決める。
///
/// `prep` は描画の前に1回だけ作った (規則, 下ごしらえ) の列
/// ([`CondRule::aux`] は範囲の統計を採るので、セルごとに作り直さない)。
///
/// **後に来た規則が勝つ。** xlsx の `priority` は読みの時点で並べ替え済みで、
/// ここは並びをそのまま信じる。当たった規則が塗りを持たなければ塗りは
/// 触らない(前の規則の答えが残る)— これも `CondLook` の三択と同じ考え方
pub fn resolve_cond(prep: &[(CondRule, CondAux)], p: Pos, v: &Value) -> CondResolved {
    let mut out = CondResolved::default();
    for (rule, aux) in prep {
        if rule.hits(p, v, aux) {
            let lk = &rule.look;
            if let Some(f) = &lk.fill {
                out.fill = Some(f.clone());
            }
            if let Some(c) = &lk.color {
                out.color = Some(c.clone());
            }
            out.bold = lk.bold.or(out.bold);
            out.italic = lk.italic.or(out.italic);
            out.underline = lk.underline.or(out.underline);
            out.strike = lk.strike.or(out.strike);
        }
        // バー/スケール/アイコンは「当たり外れ」ではなく物差し。
        // hits() は必ず false を返すので、上とは排他になっている
        if let Some(t) = rule.scalar(p, v, aux) {
            match &rule.kind {
                CondKind::Bar(c) => out.bar = Some((t, c.clone())),
                CondKind::Scale(..) => {
                    if let Some(c) = rule.scale_color(t) {
                        out.fill = Some(c);
                    }
                }
                CondKind::Icons(name) => out.icon = Some(icon_of(name, t)),
                _ => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, CondLook, CondOp, Sheet};

    fn sheet_with(vals: &[(u32, f64)]) -> Sheet {
        let mut s = Sheet::default();
        for (r, x) in vals {
            s.set(Pos::new(*r, 0), Cell { value: Value::Number(*x), ..Default::default() });
        }
        s
    }

    fn prep(s: &Sheet, rules: Vec<CondRule>) -> Vec<(CondRule, CondAux)> {
        rules.into_iter().map(|r| { let a = r.aux(s); (r, a) }).collect()
    }

    fn rule(kind: CondKind, look: CondLook) -> CondRule {
        CondRule { range: (Pos::new(0, 0), Pos::new(3, 0)), kind, look }
    }

    #[test]
    fn 塗りと文字色が当たる() {
        let s = sheet_with(&[(0, 5.0), (1, 50.0)]);
        let p = prep(&s, vec![rule(
            CondKind::Cmp(CondOp::Gt, 10.0),
            CondLook { fill: Some("FF0000".into()), color: Some("FFFFFF".into()), ..Default::default() },
        )]);
        assert_eq!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(5.0)).fill, None, "当たらない値に塗りが付いた");
        let hit = resolve_cond(&p, Pos::new(1, 0), &Value::Number(50.0));
        assert_eq!(hit.fill.as_deref(), Some("FF0000"));
        assert_eq!(hit.color.as_deref(), Some("FFFFFF"));
    }

    #[test]
    fn 触らない飾りは元のまま() {
        // look が bold を持たない規則に当たっても、bold は None のまま。
        // ここが Some(false) になると、元から太字のセルが細くなる
        let s = sheet_with(&[(0, 50.0)]);
        let p = prep(&s, vec![rule(
            CondKind::Cmp(CondOp::Gt, 10.0),
            CondLook { fill: Some("FF0000".into()), ..Default::default() },
        )]);
        assert_eq!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(50.0)).bold, None);
    }

    #[test]
    fn 後の規則が勝つ() {
        let s = sheet_with(&[(0, 50.0)]);
        let p = prep(&s, vec![
            rule(CondKind::Cmp(CondOp::Gt, 10.0), CondLook { fill: Some("FF0000".into()), ..Default::default() }),
            rule(CondKind::Cmp(CondOp::Gt, 20.0), CondLook { fill: Some("00FF00".into()), ..Default::default() }),
        ]);
        assert_eq!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(50.0)).fill.as_deref(), Some("00FF00"));
    }

    #[test]
    fn 当たらない規則は前の答えを消さない() {
        let s = sheet_with(&[(0, 15.0)]);
        let p = prep(&s, vec![
            rule(CondKind::Cmp(CondOp::Gt, 10.0), CondLook { fill: Some("FF0000".into()), ..Default::default() }),
            rule(CondKind::Cmp(CondOp::Gt, 20.0), CondLook { fill: Some("00FF00".into()), ..Default::default() }),
        ]);
        assert_eq!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(15.0)).fill.as_deref(), Some("FF0000"));
    }

    #[test]
    fn データバーは範囲の中の位置() {
        let s = sheet_with(&[(0, 0.0), (1, 5.0), (2, 10.0)]);
        let p = prep(&s, vec![rule(CondKind::Bar("638EC6".into()), CondLook::default())]);
        let (t, c) = resolve_cond(&p, Pos::new(1, 0), &Value::Number(5.0)).bar.expect("棒が出ない");
        assert!((t - 0.5).abs() < 1e-9, "真ん中の値が {t}");
        assert_eq!(c, "638EC6");
        assert_eq!(resolve_cond(&p, Pos::new(2, 0), &Value::Number(10.0)).bar.unwrap().0, 1.0);
    }

    #[test]
    fn カラースケールは塗りとして返る() {
        // 描く側から見れば「このセルの塗り」— バーと違って別の欄にしない
        let s = sheet_with(&[(0, 0.0), (1, 10.0)]);
        let p = prep(&s, vec![rule(
            CondKind::Scale("FF0000".into(), None, "00FF00".into()),
            CondLook::default(),
        )]);
        assert!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(0.0)).fill.is_some());
        assert!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(0.0)).bar.is_none());
    }

    #[test]
    fn アイコンは3段() {
        let s = sheet_with(&[(0, 0.0), (1, 5.0), (2, 10.0)]);
        let p = prep(&s, vec![rule(CondKind::Icons("3Arrows".into()), CondLook::default())]);
        assert_eq!(resolve_cond(&p, Pos::new(0, 0), &Value::Number(0.0)).icon.unwrap().0, "↓");
        assert_eq!(resolve_cond(&p, Pos::new(1, 0), &Value::Number(5.0)).icon.unwrap().0, "→");
        assert_eq!(resolve_cond(&p, Pos::new(2, 0), &Value::Number(10.0)).icon.unwrap().0, "↑");
        // 矢印でないセットは信号の丸。**色は3段で違う**
        let p2 = prep(&s, vec![rule(CondKind::Icons("3TrafficLights1".into()), CondLook::default())]);
        let lo = resolve_cond(&p2, Pos::new(0, 0), &Value::Number(0.0)).icon.unwrap();
        let hi = resolve_cond(&p2, Pos::new(2, 0), &Value::Number(10.0)).icon.unwrap();
        assert_eq!((lo.0, hi.0), ("●", "●"));
        assert_ne!(lo.1, hi.1);
    }

    #[test]
    fn 範囲の外は何も返さない() {
        let s = sheet_with(&[(0, 50.0)]);
        let p = prep(&s, vec![rule(
            CondKind::Cmp(CondOp::Gt, 10.0),
            CondLook { fill: Some("FF0000".into()), ..Default::default() },
        )]);
        // 規則の範囲は A1:A4 — B1 は外
        assert_eq!(resolve_cond(&p, Pos::new(0, 1), &Value::Number(50.0)), CondResolved::default());
    }
}
