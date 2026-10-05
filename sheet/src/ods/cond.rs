//! Conditional formats of an ods.
//!
//! LibreOffice writes them in its own extension, `calcext:conditional-formats`
//! at the end of a table: one `calcext:conditional-format` per range, holding
//! `calcext:condition` rules (an operator and a value, or a function such as
//! `top-elements(3)`), a `calcext:data-bar`, a `calcext:color-scale` or a
//! `calcext:icon-set`. A rule's look is a named cell style, given by its
//! display name. The value grammar here follows what LibreOffice 24.2 writes
//! (seen on the corpus and in sc/source/filter/xml) and what Euro-Office core
//! reads (OdfFile/Reader/Converter/xlsx_conditionalFormatting.cpp).
//!
//! The model keeps comparisons with numbers; a comparison with text or with
//! a formula becomes a formula rule anchored at the range's top-left cell.

use book::{CondKind, CondOp, Pos};

/// A condition's value as the model's rule kind, None for kinds the model
/// does not have. `anchor` is the range's top-left cell, which a comparison
/// with text is written against
pub(super) fn kind_of(value: &str, anchor: Pos) -> Option<CondKind> {
    let v = value.trim();
    let arg = |name: &str| -> Option<&str> { v.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')') };
    let num = |s: &str| s.trim().parse::<f64>().ok();
    let formula = |s: &str| super::formula::to_a1(&format!("of:={s}"));
    match v {
        "duplicate" => return Some(CondKind::Dup(false)),
        "unique" => return Some(CondKind::Dup(true)),
        "above-average" => return Some(CondKind::Avg(false)),
        "below-average" => return Some(CondKind::Avg(true)),
        _ => {}
    }
    if let Some(n) = arg("top-elements").and_then(num) {
        return Some(CondKind::Top(n as u32, false));
    }
    if let Some(n) = arg("bottom-elements").and_then(num) {
        return Some(CondKind::Top(n as u32, true));
    }
    if let Some(f) = arg("formula-is") {
        return formula(f).map(CondKind::Formula);
    }
    if let Some(t) = arg("contains-text") {
        let t = t.trim();
        let t = t.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(t).replace("\"\"", "\"");
        return Some(CondKind::Text(t));
    }
    for (name, outside) in [("between", false), ("not-between", true)] {
        if let Some(inner) = arg(name) {
            let (a, b) = split_args(inner)?;
            return match (num(a), num(b)) {
                (Some(lo), Some(hi)) => Some(CondKind::Between(lo.min(hi), lo.max(hi), outside)),
                // Bounds given as formulas: the same test as a formula rule
                _ => {
                    let c = anchor.a1();
                    let (a, b) = (formula(a)?, formula(b)?);
                    let f = if outside {
                        format!("OR({c}<MIN({a},{b}),{c}>MAX({a},{b}))")
                    } else {
                        format!("AND({c}>=MIN({a},{b}),{c}<=MAX({a},{b}))")
                    };
                    Some(CondKind::Formula(f))
                }
            };
        }
    }
    // A comparison: an operator, then a number, text or a formula
    let (op, rest) = [("<=", CondOp::Le), (">=", CondOp::Ge), ("!=", CondOp::Ne), ("<", CondOp::Lt), (">", CondOp::Gt), ("=", CondOp::Eq)]
        .into_iter()
        .find_map(|(s, op)| v.strip_prefix(s).map(|r| (op, r)))?;
    match num(rest) {
        Some(n) => Some(CondKind::Cmp(op, n)),
        None => {
            let rhs = formula(rest.trim())?;
            let sym = match op {
                CondOp::Le => "<=",
                CondOp::Ge => ">=",
                CondOp::Ne => "<>",
                CondOp::Lt => "<",
                CondOp::Gt => ">",
                CondOp::Eq => "=",
            };
            Some(CondKind::Formula(format!("{}{sym}{rhs}", anchor.a1())))
        }
    }
}

/// `10,45` or `"a";"b"` → the two arguments, split at the top level
fn split_args(s: &str) -> Option<(&str, &str)> {
    let (mut depth, mut quoted) = (0i32, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' | ';' if !quoted && depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

/// A rule kind as a `calcext:condition` value, None for the kinds written
/// as a data bar, colour scale or icon set
pub(super) fn value_of(kind: &CondKind) -> Option<String> {
    let n = |x: f64| format!("{x}");
    Some(match kind {
        CondKind::Cmp(op, x) => {
            let s = match op {
                CondOp::Gt => ">",
                CondOp::Lt => "<",
                CondOp::Eq => "=",
                CondOp::Ge => ">=",
                CondOp::Le => "<=",
                CondOp::Ne => "!=",
            };
            format!("{s}{}", n(*x))
        }
        CondKind::Between(lo, hi, false) => format!("between({},{})", n(*lo), n(*hi)),
        CondKind::Between(lo, hi, true) => format!("not-between({},{})", n(*lo), n(*hi)),
        CondKind::Text(t) => format!("contains-text(\"{}\")", t.replace('"', "\"\"")),
        CondKind::Dup(false) => "duplicate".into(),
        CondKind::Dup(true) => "unique".into(),
        CondKind::Top(k, false) => format!("top-elements({k})"),
        CondKind::Top(k, true) => format!("bottom-elements({k})"),
        CondKind::Avg(false) => "above-average".into(),
        CondKind::Avg(true) => "below-average".into(),
        CondKind::Formula(f) => {
            let of = super::formula::to_of(f);
            format!("formula-is({})", of.strip_prefix("of:=").unwrap_or(&of))
        }
        CondKind::Bar(_) | CondKind::Scale(..) | CondKind::Icons(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_libreoffice_writes_become_rules_and_back() {
        let at = Pos::new(1, 0);
        for (v, k) in [
            (">50", CondKind::Cmp(CondOp::Gt, 50.0)),
            ("between(10,45)", CondKind::Between(10.0, 45.0, false)),
            ("duplicate", CondKind::Dup(false)),
            ("top-elements(3)", CondKind::Top(3, false)),
            ("above-average", CondKind::Avg(false)),
            ("contains-text(\"品目1\")", CondKind::Text("品目1".into())),
            ("formula-is(LEN([.C2])>3)", CondKind::Formula("LEN(C2)>3".into())),
        ] {
            assert_eq!(kind_of(v, at), Some(k.clone()), "{v}");
            assert_eq!(value_of(&k).as_deref(), Some(v), "{v}");
        }
    }

    #[test]
    fn a_comparison_with_text_is_a_formula_on_the_anchor() {
        assert_eq!(kind_of("=\"Desktop\"", Pos::new(1, 2)), Some(CondKind::Formula("C2=\"Desktop\"".into())));
    }
}
