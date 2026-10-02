//! OpenFormula (ODF 1.2 Part 2) to the A1 form the engine calculates.
//!
//! An ods keeps a formula as `of:=SUM([.A1:.B2];[Sheet2.C3])`: references in
//! square brackets with the sheet before a dot, `;` between arguments, and
//! `|` between the rows of an inline array. The engine keeps formulas the way
//! an xlsx does, without the leading `=`: `SUM(A1:B2,Sheet2!C3)`.
//!
//! Function names that LibreOffice writes with a vendor prefix for an Excel
//! function (`COM.MICROSOFT.TEXTJOIN`) lose the prefix. Other prefixed names
//! are left as they are, so the engine says #NAME? for them instead of
//! calculating something else.

/// Turn an ods `table:formula` into an engine formula (no leading `=`).
/// None when the text is not a formula this reader understands (an unknown
/// namespace such as `ooow:`), so the caller keeps the cached value only
pub fn to_a1(src: &str) -> Option<String> {
    let body = strip_namespace(src)?;
    let body = body.strip_prefix('=').unwrap_or(body);
    let mut out = String::with_capacity(body.len());
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    // Depth of `{…}`: inside an inline array `;` separates columns and `|` rows
    let mut brace = 0usize;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                // A string literal; `""` is a quote inside it
                out.push('"');
                i += 1;
                while i < chars.len() {
                    out.push(chars[i]);
                    if chars[i] == '"' {
                        if chars.get(i + 1) == Some(&'"') {
                            out.push('"');
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            '[' => {
                let start = i + 1;
                let mut j = start;
                let mut quoted = false;
                while j < chars.len() {
                    match chars[j] {
                        '\'' => quoted = !quoted,
                        ']' if !quoted => break,
                        _ => {}
                    }
                    j += 1;
                }
                let inner: String = chars[start..j.min(chars.len())].iter().collect();
                out.push_str(&reference(&inner));
                i = j + 1;
            }
            '{' => {
                brace += 1;
                out.push('{');
                i += 1;
            }
            '}' => {
                brace = brace.saturating_sub(1);
                out.push('}');
                i += 1;
            }
            ';' => {
                out.push(',');
                i += 1;
            }
            '|' if brace > 0 => {
                out.push(';');
                i += 1;
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                // A name: a function, a defined name or TRUE/FALSE
                let mut j = i;
                while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '.')) {
                    j += 1;
                }
                let name: String = chars[i..j].iter().collect();
                out.push_str(function_name(&name));
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Some(out)
}

/// `of:=…` and a bare `=…` are OpenFormula. `msoxl:=…` is Excel's own syntax
/// in an ods; its A1 references are already what the engine reads
fn strip_namespace(src: &str) -> Option<&str> {
    let s = src.trim_start();
    if s.starts_with('=') {
        return Some(s);
    }
    let (ns, rest) = s.split_once(':')?;
    match ns {
        "of" | "msoxl" => Some(rest),
        _ => None,
    }
}

fn function_name(name: &str) -> &str {
    name.strip_prefix("COM.MICROSOFT.").unwrap_or(name)
}

/// The inside of `[…]`: one cell `.A1`, a range `.A1:.B2`, with an optional
/// sheet (`Sheet2.A1`, `$'My sheet'.A1`). A range across sheets
/// (`Sheet1.A1:Sheet3.A1`) becomes the 3-D form `Sheet1:Sheet3!A1`
fn reference(inner: &str) -> String {
    let parts = split_top(inner, ':');
    let parsed: Vec<(Option<String>, String)> = parts.iter().map(|p| one(p)).collect();
    match parsed.as_slice() {
        [(sheet, cell)] => with_sheet(sheet.as_deref(), cell),
        [(s1, c1), (s2, c2)] => match (s1, s2) {
            (Some(a), Some(b)) if a != b && c1 == c2 => {
                format!("{}:{}!{}", quote(a), quote(b), c1)
            }
            (Some(a), Some(b)) if a != b => {
                // Different cells on different sheets has no A1 spelling;
                // keep the first sheet so the formula still parses
                format!("{}!{}:{}", quote(a), c1, c2)
            }
            _ => with_sheet(s1.as_deref().or(s2.as_deref()), &format!("{c1}:{c2}")),
        },
        _ => inner.to_string(),
    }
}

fn with_sheet(sheet: Option<&str>, cell: &str) -> String {
    match sheet {
        Some(s) => format!("{}!{}", quote(s), cell),
        None => cell.to_string(),
    }
}

/// One side of a reference: (sheet, cell). The sheet is empty before a
/// leading dot (`.A1` is on the formula's own sheet)
fn one(p: &str) -> (Option<String>, String) {
    let p = p.trim();
    // The last dot outside quotes separates the sheet from the cell
    let mut quoted = false;
    let mut dot = None;
    for (i, ch) in p.char_indices() {
        match ch {
            '\'' => quoted = !quoted,
            '.' if !quoted => dot = Some(i),
            _ => {}
        }
    }
    let Some(d) = dot else { return (None, p.to_string()) };
    let sheet = p[..d].trim_start_matches('$');
    let cell = p[d + 1..].to_string();
    if sheet.is_empty() {
        return (None, cell);
    }
    let sheet = match sheet.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        Some(q) => q.replace("''", "'"),
        None => sheet.to_string(),
    };
    (Some(sheet), cell)
}

fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut last = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '\'' => quoted = !quoted,
            c if c == sep && !quoted => {
                out.push(&s[last..i]);
                last = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[last..]);
    out
}

/// A sheet name needs quotes when it is not one plain word. Japanese names
/// such as `4月` are written bare, as Excel writes them
fn quote(name: &str) -> String {
    let plain = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !name.chars().next().is_some_and(|c| c.is_ascii_digit());
    let plain_ja = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_ascii();
    if plain || plain_ja {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

#[cfg(test)]
mod tests {
    use super::to_a1;

    #[test]
    fn cells_and_ranges_on_the_same_sheet() {
        assert_eq!(to_a1("of:=[.F18]").as_deref(), Some("F18"));
        assert_eq!(to_a1("of:=SUM([.B11:.B17])").as_deref(), Some("SUM(B11:B17)"));
        assert_eq!(to_a1("of:=[.$A$1]*2").as_deref(), Some("$A$1*2"));
    }

    #[test]
    fn arguments_are_separated_by_commas_but_strings_are_kept() {
        assert_eq!(
            to_a1(r#"of:=IF([.C3]>0;"a;b";1)"#).as_deref(),
            Some(r#"IF(C3>0,"a;b",1)"#)
        );
        assert_eq!(to_a1(r#"of:="say ""hi"";""#).as_deref(), Some(r#""say ""hi"";""#));
    }

    #[test]
    fn other_sheets_and_quoted_names() {
        assert_eq!(to_a1("of:=[Sheet2.C3]").as_deref(), Some("Sheet2!C3"));
        assert_eq!(to_a1("of:=[$Sheet2.$C$3]").as_deref(), Some("Sheet2!$C$3"));
        assert_eq!(to_a1("of:=SUM(['4月 実績'.B1:.B5])").as_deref(), Some("SUM('4月 実績'!B1:B5)"));
        assert_eq!(to_a1("of:=[4月.B2]").as_deref(), Some("4月!B2"));
        assert_eq!(to_a1("of:=['it''s'.A1]").as_deref(), Some("'it''s'!A1"));
    }

    #[test]
    fn a_range_across_sheets_becomes_three_d() {
        assert_eq!(to_a1("of:=SUM([Apr.B2:Jun.B2])").as_deref(), Some("SUM(Apr:Jun!B2)"));
    }

    #[test]
    fn inline_arrays_swap_their_separators() {
        assert_eq!(to_a1("of:={1;2|3;4}").as_deref(), Some("{1,2;3,4}"));
    }

    #[test]
    fn excel_prefixes_are_dropped_and_unknown_namespaces_refused() {
        assert_eq!(
            to_a1("of:=COM.MICROSOFT.TEXTJOIN(\",\";1;[.A1:.A3])").as_deref(),
            Some("TEXTJOIN(\",\",1,A1:A3)")
        );
        assert_eq!(to_a1("ooow:=1+1"), None);
    }
}
