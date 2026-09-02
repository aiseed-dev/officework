//! 本家の試験を写した試験に共通の道具
use kumihan::{Document, Run};

/// run の並びを本家の HTML と同じ形にする。
///
/// 印は開いた順に外へ置き、閉じるときは内側から閉じます(本家の
/// 出力と同じ)。役割は、その run だけで開いて閉じる印があればその印の
/// `class` に、無ければ `span` に付けます。id は模型に無いので出ません
pub fn sig(runs: &[Run]) -> String {
    #[derive(Clone, PartialEq)]
    struct Tag(String, Option<String>); // (名前, class)
    fn bare_tags(r: &Run) -> Vec<String> {
        let f = &r.fmt;
        let mut v: Vec<String> = Vec::new();
        if f.bold {
            v.push("strong".into());
        }
        if f.italic {
            v.push("em".into());
        }
        if f.style_id.as_deref() == Some("等幅") {
            v.push("code".into());
        }
        if f.highlight.is_some() {
            v.push("mark".into());
        }
        if f.superscript {
            v.push("sup".into());
        }
        if f.subscript {
            v.push("sub".into());
        }
        // リンクは一番内側(`*see https://…*` の形が多い)
        if let Some(u) = &f.link {
            v.push(format!("a href=\"{u}\""));
        }
        if let Some(r) = &f.field {
            v.push(format!("a href=\"#{}\"", r.name));
        }
        v
    }
    let role_of = |r: &Run| {
        r.fmt.style_id.as_deref().filter(|s| *s != "等幅").map(|s| s.replace('.', " "))
    };
    let tags_of = |i: usize| -> Vec<Tag> {
        let r = &runs[i];
        let bare = bare_tags(r);
        let mut v: Vec<Tag> = bare.iter().map(|n| Tag(n.clone(), None)).collect();
        if let Some(role) = role_of(r) {
            // この run だけの印(前後の run に無い印)があれば、そこに class
            let prev = i.checked_sub(1).map(|k| bare_tags(&runs[k])).unwrap_or_default();
            let next = runs.get(i + 1).map(bare_tags).unwrap_or_default();
            let own = v.iter_mut().rev().find(|t| !prev.contains(&t.0) && !next.contains(&t.0));
            match own {
                Some(t) => t.1 = Some(role),
                None => v.push(Tag("span".into(), Some(role))),
            }
        }
        v
    };
    let mut out = String::new();
    let mut stack: Vec<Tag> = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        let want = tags_of(i);
        while let Some(top) = stack.last() {
            if want.contains(top) {
                break;
            }
            let name = top.0.split(' ').next().unwrap_or("").to_string();
            out.push_str(&format!("</{name}>"));
            stack.pop();
        }
        for t in &want {
            if !stack.contains(t) {
                match &t.1 {
                    Some(c) => out.push_str(&format!("<{} class=\"{c}\">", t.0)),
                    None => out.push_str(&format!("<{}>", t.0)),
                }
                stack.push(t.clone());
            }
        }
        out.push_str(&r.text);
    }
    while let Some(top) = stack.pop() {
        let name = top.0.split(' ').next().unwrap_or("").to_string();
        out.push_str(&format!("</{name}>"));
    }
    out
}

pub fn first_runs(d: &Document) -> Vec<Run> {
    d.paragraphs().next().map(|p| p.runs.clone()).unwrap_or_default()
}

