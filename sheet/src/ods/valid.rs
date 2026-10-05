//! Data validation of an ods.
//!
//! ODF keeps the rules once, in `table:content-validations` before the
//! tables, and each cell names the rule it uses
//! (`table:content-validation-name`). A rule's `table:condition` is
//! OpenFormula with its own functions (ODF 1.3 Part 3, 19.619):
//! `of:cell-content-is-in-list("a";"b")`,
//! `of:cell-content-is-whole-number() and cell-content-is-between(1,100)`,
//! `of:cell-content-text-length()<=8`, `of:is-true-formula(…)`.
//!
//! The model keeps the rule the way an xlsx does: a type (`list`,
//! `whole`, `decimal`, `textLength`, `date`, `time`, `custom`), an operator
//! and one or two formulas.

/// A condition as (type, operator, formula1, formula2), None when the
/// condition is not one this understands
pub(super) fn parse(cond: &str) -> Option<(String, String, String, String)> {
    let c = cond.trim();
    let c = c.strip_prefix("of:").or_else(|| c.strip_prefix("ooo:")).unwrap_or(c).trim();
    let f = |s: &str| super::formula::to_a1(&format!("of:={}", s.trim()));
    if let Some(inner) = call(c, "cell-content-is-in-list") {
        // Either quoted items or one range
        let items = split_top(inner, ';');
        let quoted = items.iter().all(|i| i.trim().starts_with('"'));
        let formula = if quoted {
            let words: Vec<String> = items
                .iter()
                .map(|i| i.trim().trim_matches('"').replace("\"\"", "\""))
                .collect();
            format!("\"{}\"", words.join(","))
        } else {
            f(inner)?
        };
        return Some(("list".into(), String::new(), formula, String::new()));
    }
    if let Some(inner) = call(c, "is-true-formula") {
        return Some(("custom".into(), String::new(), f(inner)?, String::new()));
    }
    // `type() and comparison`, or a text length comparison alone
    let (kind, rest) = if let Some(r) = c.strip_prefix("cell-content-text-length") {
        ("textLength", format!("cell-content{r}"))
    } else {
        let (head, rest) = c.split_once(" and ")?;
        let kind = match head.trim() {
            "cell-content-is-whole-number()" => "whole",
            "cell-content-is-decimal-number()" => "decimal",
            "cell-content-is-date()" => "date",
            "cell-content-is-time()" => "time",
            _ => return None,
        };
        (kind, rest.trim().to_string())
    };
    let rest = rest.as_str();
    for (name, op) in [("cell-content-is-between", "between"), ("cell-content-is-not-between", "notBetween")] {
        if let Some(inner) = call(rest, name) {
            let parts = split_top(inner, ',');
            let parts = if parts.len() == 2 { parts } else { split_top(inner, ';') };
            let [a, b] = parts.as_slice() else { return None };
            return Some((kind.into(), op.into(), f(a)?, f(b)?));
        }
    }
    let after = rest.strip_prefix("cell-content()")?.trim();
    // `==` is what Euro-Office and ONLYOFFICE write for equal; LibreOffice
    // reads it as `=` too
    let (op, value) = [("==", "equal"), ("<=", "lessThanOrEqual"), (">=", "greaterThanOrEqual"), ("!=", "notEqual"), ("<", "lessThan"), (">", "greaterThan"), ("=", "equal")]
        .into_iter()
        .find_map(|(s, op)| after.strip_prefix(s).map(|v| (op, v)))?;
    Some((kind.into(), op.into(), f(value)?, String::new()))
}

/// `name(inner)` → inner
fn call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    s.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

fn split_top(s: &str, sep: char) -> Vec<&str> {
    let (mut depth, mut quoted, mut bracket) = (0i32, false, false);
    let mut out = Vec::new();
    let mut last = 0;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            '(' if !quoted && !bracket => depth += 1,
            ')' if !quoted && !bracket => depth -= 1,
            c if c == sep && !quoted && !bracket && depth == 0 => {
                out.push(&s[last..i]);
                last = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[last..]);
    out
}

/// A rule as a `table:condition`, None for types ODF has no condition for
pub(super) fn condition(v: &book::Validation) -> Option<String> {
    let of = |f: &str| {
        let o = super::formula::to_of(f);
        o.strip_prefix("of:=").unwrap_or(&o).to_string()
    };
    if v.kind == "list" {
        let f = v.formula.trim();
        // `"a,b,c"` is the items; anything else is a range or a name
        let inner = match f.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
            Some(items) => items.split(',').map(|i| format!("\"{}\"", i.replace('"', "\"\""))).collect::<Vec<_>>().join(";"),
            None => of(f),
        };
        return Some(format!("of:cell-content-is-in-list({inner})"));
    }
    if v.kind == "custom" {
        return Some(format!("of:is-true-formula({})", of(&v.formula)));
    }
    let head = match v.kind.as_str() {
        "whole" => "cell-content-is-whole-number() and ",
        "decimal" => "cell-content-is-decimal-number() and ",
        "date" => "cell-content-is-date() and ",
        "time" => "cell-content-is-time() and ",
        "textLength" => "",
        _ => return None,
    };
    let subject = if v.kind == "textLength" { "cell-content-text-length" } else { "cell-content" };
    let test = match v.op.as_str() {
        "between" | "" => format!("{subject}-is-between({},{})", of(&v.formula), of(&v.formula2)),
        "notBetween" => format!("{subject}-is-not-between({},{})", of(&v.formula), of(&v.formula2)),
        op => {
            let sym = match op {
                "equal" => "=",
                "notEqual" => "!=",
                "greaterThan" => ">",
                "lessThan" => "<",
                "greaterThanOrEqual" => ">=",
                "lessThanOrEqual" => "<=",
                _ => return None,
            };
            format!("{subject}(){sym}{}", of(&v.formula))
        }
    };
    Some(format!("of:{head}{test}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: &str, op: &str, f1: &str, f2: &str) -> book::Validation {
        book::Validation {
            range: Default::default(),
            formula: f1.into(),
            kind: kind.into(),
            op: op.into(),
            formula2: f2.into(),
            input_msg: None,
            error_msg: None,
            allow_blank: true,
            hide_arrow: false,
        }
    }

    #[test]
    fn conditions_libreoffice_writes_are_read_and_written_back() {
        for (c, (k, op, f1, f2)) in [
            ("of:cell-content-is-in-list(\"甲\";\"乙\";\"丙\")", ("list", "", "\"甲,乙,丙\"", "")),
            ("of:cell-content-is-in-list([.$H$2:.$H$40])", ("list", "", "$H$2:$H$40", "")),
            ("of:cell-content-is-whole-number() and cell-content-is-between(1,100)", ("whole", "between", "1", "100")),
            ("of:cell-content-is-decimal-number() and cell-content()>0.5", ("decimal", "greaterThan", "0.5", "")),
            ("of:cell-content-text-length()<=8", ("textLength", "lessThanOrEqual", "8", "")),
            ("of:is-true-formula(LEN([.A2])>3)", ("custom", "", "LEN(A2)>3", "")),
        ] {
            let got = parse(c).unwrap_or_else(|| panic!("{c}"));
            assert_eq!(got, (k.to_string(), op.to_string(), f1.to_string(), f2.to_string()), "{c}");
            assert_eq!(condition(&rule(k, op, f1, f2)).as_deref(), Some(c), "{c}");
        }
    }

    #[test]
    fn a_double_equals_sign_is_read_as_equal() {
        let got = parse("of:cell-content-is-whole-number() and cell-content()==5");
        assert_eq!(got, Some(("whole".into(), "equal".into(), "5".into(), String::new())));
        let got = parse("of:cell-content-text-length()==3");
        assert_eq!(got, Some(("textLength".into(), "equal".into(), "3".into(), String::new())));
    }
}
