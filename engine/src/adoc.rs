//! **AsciiDoc の読み書き** — ネイティブ文書の保存形式(段階B)。
//!
//! SEKKEI「本文とテンプレートを分ける」(2026-08-16 発注者): ネイティブ文書は
//! 意味だけを AsciiDoc で持ち、見た目はテンプレート([`crate::theme`])が持つ。
//! ここが読むのは **AsciiDoc の部分集合** — SEKKEI の対応表が正本。
//!
//! # 門番
//!
//! `write(parse(src)) == src`(恒等)。書きは常に**正規形**を出す
//! (段落は1行・区切りは空行1つ)ので、正規形どうしで恒等が成り立つ。
//!
//! # 部分集合の決め
//!
//! - 段落の中の行の継ぎ方は AsciiDoc と違い**空白を挟まない**(日本語の
//!   文を行で折っても語が割れないように)。書きは1段落=1行で出すので、
//!   往復には影響しない
//! - 表のセルは1行=1行(docx の縦結合は `.N+`、横結合は `N+` の頭書き)
//! - ルビは自前のインライン `ruby:字[よみ]`(AsciiDoc に無い。規約)
//! - 記入欄は `field:タグ[表示名,種類]`(2026-08-17 に足した)。**記入欄は
//!   意味**です — 「ここに名前を書く」という指示であって見た目ではないので、
//!   意味だけの本文に書けます。アプリの形(HTML の form)で書き出す土台
//! - ペン・変更履歴は**ここに無い** — 互換モード(docx)の機能
//!   (SEKKEI の決め。歴史とコメントは git)
//!
//! # 扱わない書き方は帳簿に出す
//!
//! 本家の AsciiDoc には、うちが扱わない書き方がたくさんあります(註記・
//! コードの塊・取り込み・属性の参照など)。それらは**字としては本文に残り、
//! 意味は落ちます**。[`parse_full`] が落ちた物の一覧を返し、writer が画面に
//! 出します。**2026-08-18 まで黙って化けていました** — この註の上のほうに
//! 「知らない書き方は Err で言う」と書いてありましたが、8つ試して8つとも
//! 黙って本文になっていました。書いてあることと実物が違っていたので、
//! 実物のほうを直しました

use crate::doc::{
    Block, Cellbox, CharFormat, Document, InlineImage, ListKind, Paragraph, ParaStyle, Run,
    Table, VMerge,
};

// ---- 書き ------------------------------------------------------------------

/// 模型 → AsciiDoc(正規形)。**意味だけを書く** — 見た目の欄
/// (size_pt・font・色…)は見ない。ネイティブ文書では常に空のはずで、
/// docx をそのまま通すと、本文に直に付いた書式は落ちる(書式を別ファイルへ
/// 移すのは distill の仕事)
pub fn write(doc: &Document) -> String {
    let mut out = String::new();
    // 頭(題名と属性)。**属性は読んだ順に返します** — 並べ替えると、
    // 書いた人の差分が「全部変わった」に見えます
    let mut head = String::new();
    // 表題は本文の先頭の段落(ParaStyle::Title)から出します。段落が無く
    // 文書の情報にだけ題名があるとき(docx から来た文書など)はそちらから
    let title_para = matches!(doc.blocks.first(), Some(Block::Para(p)) if p.style == ParaStyle::Title);
    if title_para {
        if let Some(Block::Para(p)) = doc.blocks.first() {
            head.push_str(&format!("= {}\n", runs_text(&p.runs, doc)));
        }
    } else if !doc.props.title.is_empty() {
        head.push_str(&format!("= {}\n", doc.props.title));
    }
    let name = |k: &str| k == "template" || k == "テンプレート";
    if let Some(t) = &doc.template {
        if !doc.attrs.iter().any(|(k, _)| name(k)) {
            // このアプリが後から名前を付けた(読んだ字には無かった)
            head.push_str(&format!(":template: {t}\n"));
        }
    }
    // 節のある文書は、最初の節の用紙を頭に書きます(`:page-size:` など)。
    // 読んだ値ではなく**いまの用紙**を書きます — 画面で変えた分が保存で消えないように
    let geo = crate::layout::section_geometry(doc);
    let mut page_head_done = false;
    let mut push_page_head = |head: &mut String| {
        if page_head_done { return; }
        page_head_done = true;
        for (k, v) in page_attr_pairs(&geo[0]) {
            head.push_str(&format!(":{k}: {v}\n"));
        }
    };
    for (k, v) in &doc.attrs {
        if k.is_empty() {
            head.push_str(&format!("{v}\n")); // 原文のままの行(著者の行など)
            continue;
        }
        if !geo.is_empty() && is_page_attr_key(k) {
            push_page_head(&mut head);
            continue;
        }
        // テンプレートの名前は後から変わりうるので、いまの値で書きます
        let v = if name(k) { doc.template.clone().unwrap_or_else(|| v.clone()) } else { v.clone() };
        // 値の無い属性は `:名前:` と書く(後ろに空白を足さない)
        if v.is_empty() {
            head.push_str(&format!(":{k}:\n"));
        } else {
            head.push_str(&format!(":{k}: {v}\n"));
        }
    }
    if !geo.is_empty() {
        push_page_head(&mut head);
    }
    if !head.is_empty() {
        out.push_str(&head);
        out.push('\n');
    }
    let mut quote_open = false;
    for (bi, b) in doc.blocks.iter().enumerate() {
        if bi == 0 && title_para {
            continue; // 頭で書いた
        }
        // **節の切れ目。** 前の段落で節が終わっていれば、ここからの用紙を
        // `[page-…]` で書き、改ページする節なら `<<<` を続けます
        let prev_sect = match bi.checked_sub(1).map(|i| &doc.blocks[i]) {
            Some(Block::Para(q)) => q.sect.as_ref(),
            _ => None,
        };
        if let Some(sb) = prev_sect {
            if quote_open {
                out.push_str("____\n\n");
                quote_open = false;
            }
            out.push_str(&page_attr_line(&geo[bi]));
            out.push('\n');
            if !sb.continuous {
                out.push_str("<<<\n");
            }
            out.push('\n');
        }
        match b {
            Block::Para(p) => {
                // 節の印だけを持つ空の段落は、上の切れ目の行が代わりです
                if p.runs.is_empty() && p.sect.is_some() && p.raw_adoc.is_none() && p.images_new.is_empty() && p.images.is_empty() {
                    continue;
                }
                let is_quote = p.style == ParaStyle::Quote;
                if quote_open && !is_quote {
                    out.push_str("____\n\n");
                    quote_open = false;
                }
                if is_quote && !quote_open {
                    out.push_str("____\n");
                    quote_open = true;
                }
                // 同じ種類のリストが続く間は空行を挟まない(1つのリスト)。
                // ラベル付きリスト(`項目:: 値`)も同じ — 空行を入れると
                // 1つの一覧が2つに割れます(2026-08-18)
                let desc = p.style_id.as_deref().is_some_and(is_desc_list);
                let next_is_cont = matches!(doc.blocks.get(bi + 1), Some(Block::Para(q))
                    if matches!(q.style_id.as_deref(), Some("一覧の続き") | Some("覚え書き") | Some("コールアウト") | Some("指定の行")));
                // 項目だけの `term::` の直後の一覧は、その項目の説明(空行を挟まない)
                let term_then_list = desc
                    && desc_term_only(&runs_text(&p.runs, doc))
                    && matches!(doc.blocks.get(bi + 1), Some(Block::Para(q)) if q.list != ListKind::None);
                let tight = next_is_cont
                    || term_then_list
                    || (p.list != ListKind::None || desc)
                        && matches!(
                            doc.blocks.get(bi + 1),
                            Some(Block::Para(q))
                                if q.list == p.list
                                    && q.style_id.as_deref().is_some_and(is_desc_list) == desc
                                    // **次が新しい一覧の始めなら、空行を残します**
                                    && q.style_id.as_deref() != Some(DESC_LIST_START)
                        );
                write_para(&mut out, p, doc, quote_open || tight);
            }
            Block::Table(t) => {
                if quote_open {
                    out.push_str("____\n\n");
                    quote_open = false;
                }
                write_table(&mut out, t, doc);
            }
        }
    }
    if quote_open {
        out.push_str("____\n\n");
    }
    // 末尾は空行1つに揃える(正規形)
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn write_para(out: &mut String, p: &Paragraph, doc: &Document, in_quote: bool) {
    // **原文のまま持ち越した行は、そのまま返します。**
    // 空行を入れないのは、`----` の塊の中で行が離れてしまうからです
    if let Some(raw) = &p.raw_adoc {
        out.push_str(raw);
        out.push('\n');
        return;
    }
    if p.page_break_before {
        out.push_str("<<<\n\n");
    }
    for bm in &p.bookmarks {
        out.push_str(&format!("[[{bm}]]\n"));
    }
    // **段落のスタイル名**(2026-08-16)。AsciiDoc の塊の属性の書き方。
    // 書いていなかったので、右パネルで着せた名前が保存で黙って消えていた
    // (実機で見つけた — 試験は合成しか見ていなかった)
    if let Some(n) = &p.style_id {
        // **塊の中は字のまま1行で書きます。** 行の中の書き方として
        // 読んでいないので、書くときも印を付けません。空行もそのまま
        if is_block_inner(n) {
            for r in &p.runs {
                out.push_str(&r.text);
            }
            out.push('\n');
            return;
        }
        // ラベル付きリストは字に `::` が入っているので、名前は書かない。
        // 一覧の始めの印も同じ — *印は空行を残すためだけ*の物なので、
        // 字には出しません
        if is_desc_list(n) {
            out.push_str(&runs_text(&p.runs, doc));
            out.push('\n');
            if !in_quote {
                out.push('\n');
            }
            return;
        }
        // 註記は本家の印で書く(`[.註記]` ではなく `NOTE: `)
        if let Some(mark) = admon_mark(n) {
            out.push_str(mark);
            out.push(' ');
            out.push_str(&runs_text(&p.runs, doc));
            out.push_str("\n\n");
            return;
        }
        out.push_str(&format!("[.{n}]\n"));
    }
    // この段落が画像だけなら image:: のブロックで
    if let Some(im) = p.images_new.first().or_else(|| p.images.first()) {
        if p.runs.iter().all(|r| r.text.is_empty()) {
            if let Some(tex) = &im.tex {
                out.push_str(&format!("stem:[{tex}]\n\n"));
                return;
            }
            if let Some(src) = &im.src {
                out.push_str(&format!("image::{src}[]\n\n"));
                return;
            }
        }
    }
    let head = match (p.style, p.list) {
        (ParaStyle::Heading(n), _) => "=".repeat(n as usize + 1) + " ",
        // 読んだ印(`-`・`7.`・`i)`)があればそのまま。docx 由来の `○` は違う
        (_, ListKind::Bullet | ListKind::Number)
            if p.list_text.as_deref().is_some_and(is_adoc_list_mark) =>
        {
            format!("{} ", p.list_text.as_deref().unwrap_or_default())
        }
        (_, ListKind::Bullet) => "*".repeat(p.indent as usize + 1) + " ",
        (_, ListKind::Number) => ".".repeat(p.indent as usize + 1) + " ",
        _ => String::new(),
    };
    out.push_str(&head);
    let text = runs_text(&p.runs, doc);
    // **一文一行で書きます**(2026-08-18)。git の差分が文ごとになるので、
    // 1文直したときに何を直したのか読めます。読むときは続く行を1つの段落に
    // 継ぐので、開き直すと元の1段落に戻ります。
    //
    // 切るのは**普通の本文の段落だけ**です。見出しと箇条書きは、続く行が
    // 別の段落になってしまうので切りません
    // 段落の中の改行は ` +` で書きます(本家の書き方)
    let text = text.replace('\n', " +\n");
    let may_break = head.is_empty() && p.style == ParaStyle::Body;
    if may_break {
        out.push_str(&split_sentences(&text).join("\n"));
    } else {
        out.push_str(&text);
    }
    out.push('\n');
    if !in_quote {
        out.push('\n');
    }
}

/// **その `.` は略語の点か。**
///
/// 「Dr. Smith」の `.` の後ろは空白と大文字ですが、文の終わりではありません。
/// 頭文字1文字(`J. R. R.`)も同じです。ここで切ると、1つの名前が2行に
/// 割れます(2026-08-18 に見本で見つけました)。
fn after_abbrev(before: &[char]) -> bool {
    let word: String = before
        .iter()
        .rev()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if word.len() == 1 {
        return true; // 頭文字
    }
    const ABBREVS: &[&str] = &[
        "Dr", "Mr", "Mrs", "Ms", "Prof", "Sr", "Jr", "St", "vs", "etc", "Fig", "No", "Vol",
        "Inc", "Ltd", "Co", "Corp", "Ave", "Rd", "approx", "cf", "al",
    ];
    ABBREVS.iter().any(|x| x.eq_ignore_ascii_case(&word))
}

/// **1つの段落を、文ごとの行に切ります。**
///
/// 切るのは `。` `!` `?` の後ろです。欧文の `.` は、後ろが**空白と大文字**の
/// ときだけ切ります(`Dr.` や `example.com` で切らないため)。
///
/// `[…]` の中と `` ` `` で囲んだ中では切りません。脚注(`footnote:[…]`)の
/// 文にも `。` が入るので、そこで切ると書き方が壊れます。
fn split_sentences(s: &str) -> Vec<String> {
    let b: Vec<char> = s.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut now = String::new();
    let mut depth = 0i32;
    let mut mono = false;
    // **強調の途中では切りません**(2026-08-18)。`*太字。*` の中で切ると
    // 印が片方だけの行になり、次に開いたとき別の意味になります
    // (README を自分のエンジンに通して見つけました)
    let mut bold = false;
    let mut italic = false;
    let mut escaped = false;
    for (i, c) in b.iter().enumerate() {
        now.push(*c);
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => {
                escaped = true; // `\*` は字の `*`
                continue;
            }
            '`' => mono = !mono,
            '[' if !mono => depth += 1,
            ']' if !mono => depth = (depth - 1).max(0),
            '*' if !mono && depth == 0 => bold = !bold,
            '_' if !mono && depth == 0 => italic = !italic,
            _ => {}
        }
        if depth > 0 || mono || bold || italic {
            continue;
        }
        let cut = match c {
            '。' | '！' | '？' => true,
            '.' => {
                matches!(
                    (b.get(i + 1), b.get(i + 2)),
                    (Some(' '), Some(next_of)) if next_of.is_ascii_uppercase()
                ) && !after_abbrev(&b[..i])
            }
            _ => false,
        };
        // 行末の `.` の後ろの空白は落とす(次の行の頭には要らない)
        if cut && i + 1 < b.len() {
            out.push(std::mem::take(&mut now));
            if b.get(i + 1) == Some(&' ') {
                // 欧文の切れ目。空白は捨てて、読むときに継ぎ目で足し直す
            }
        }
    }
    if !now.is_empty() {
        out.push(now);
    }
    // **2行目からの頭の空白だけ**落とします(欧文の切れ目のぶん)。
    // 1行目の空白は書いた人の字なので残します
    out.into_iter()
        .enumerate()
        .map(|(i, x)| if i == 0 { x } else { x.trim_start().to_string() })
        .filter(|x| !x.is_empty())
        .collect()
}

/// この run の**次に来る字**(囲みの外の字を見るため)
/// run の並び → インラインの印つきの1行
fn runs_text(runs: &[Run], doc: &Document) -> String {
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Span {
        Bold,
        Italic,
        Mono,
    }
    const SPANS: [Span; 3] = [Span::Bold, Span::Italic, Span::Mono];
    let has = |r: &Run, k: Span| match k {
        Span::Bold => r.fmt.bold,
        Span::Italic => r.fmt.italic,
        Span::Mono => r.fmt.style_id.as_deref() == Some(MONO),
    };
    // 囲みの終わり(start から先で、その書式を持たない最初の run の番号)
    let end_of = |start: usize, k: Span| {
        runs[start..].iter().position(|x| !has(x, k)).map_or(runs.len(), |n| start + n)
    };
    // その run を書いたとき最初に出る字(マクロなら頭の字)
    let first_char = |r: &Run| -> Option<char> {
        if r.fmt.footnote.is_some() || r.fmt.sdt.is_some() {
            Some('f')
        } else if r.fmt.field.is_some() {
            Some('<')
        } else if r.fmt.ruby.is_some() {
            Some('r')
        } else if r.fmt.link.is_some() {
            Some('h')
        } else {
            r.text.chars().next()
        }
    };
    let is_char = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let is_space = |c: Option<char>| c.is_some_and(|c| c.is_whitespace());
    // **印を1つにするか2つにするかは、囲みの外の字と中身の端で決まります。**
    // 本家は一重の印を、外側の隣が字のときと、中身が空白で始まるか
    // 終わるときに印として読みません(2026-08-18。2026-09-02 に本家の
    // `QUOTE_SUBS` を写して条件を揃えた)。開くときと閉じるときで数が
    // 違うと対にならないので、開く前に閉じた先まで見て決めます
    let should_double = |before: Option<char>, start: usize, k: Span| -> bool {
        let end = end_of(start, k);
        let after = runs.get(end).and_then(first_char);
        let first = runs[start].text.chars().next();
        let last = runs[end - 1].text.chars().last();
        is_char(before) || is_char(after) || is_space(first) || is_space(last)
    };
    let mark_of = |k: Span, double: bool| match (k, double) {
        (Span::Bold, true) => "**",
        (Span::Bold, false) => "*",
        (Span::Italic, true) => "__",
        (Span::Italic, false) => "_",
        (Span::Mono, true) => "``",
        (Span::Mono, false) => "`",
    };
    let mut s = String::new();
    // いま開いている囲み(開いた順。閉じるのは逆順)
    let mut stack: Vec<(Span, bool)> = Vec::new();
    for (ri, r) in runs.iter().enumerate() {
        let want: Vec<Span> = SPANS.iter().copied().filter(|&k| has(r, k)).collect();
        // **閉じるのは開いた逆順です**(本家と同じ入れ子)。途中の物を
        // 閉じるときは上の物も一度閉じて、続くなら開き直します
        while stack.iter().any(|(k, _)| !want.contains(k)) {
            let (k, dbl) = stack.pop().expect("空でない");
            s.push_str(mark_of(k, dbl));
        }
        // **役割は、この run で開いてこの run で閉じる印に付けます**
        // (`[.path]_径路_`。本家の書き方)。斜体・太字の順に見て、
        // どちらも無ければ `[.名前]#字#` の形にします(2026-09-02)
        let role = r.fmt.style_id.as_deref().filter(|n| *n != MONO);
        let opens_alone = |k: Span| {
            want.contains(&k) && !stack.iter().any(|(q, _)| *q == k) && end_of(ri, k) == ri + 1
        };
        let role_on_mark: Option<Span> = match role {
            Some(_) if opens_alone(Span::Italic) => Some(Span::Italic),
            Some(_) if opens_alone(Span::Bold) => Some(Span::Bold),
            _ => None,
        };
        for k in SPANS {
            if want.contains(&k) && !stack.iter().any(|(q, _)| *q == k) {
                let dbl = should_double(s.chars().last(), ri, k);
                if role_on_mark == Some(k) {
                    s.push_str(&format!("[.{}]", role.unwrap_or_default()));
                }
                s.push_str(mark_of(k, dbl));
                stack.push((k, dbl));
            }
        }
        // 脚注の印(字を持たない run)
        if let Some(fr) = &r.fmt.footnote {
            if let Some(fnote) = doc.footnotes.iter().find(|f| f.id == fr.id && f.endnote == fr.endnote) {
                let text: String = fnote
                    .paragraphs
                    .iter()
                    .flat_map(|p| p.runs.iter())
                    .map(|r| r.text.as_str())
                    .collect();
                s.push_str(&format!("footnote:[{text}]"));
            }
            continue;
        }
        if let Some(f) = &r.fmt.field {
            // 参照。字が名前と同じなら `<<名前>>`、違えば `<<名前,字>>`
            let _ = f.page;
            // URL やパスの形の名前(`://` `.` `/` `#` を含む)は `xref:名前[字]` で
            // 書きます。`<<URL>>` は本家がリンクに読み、`<<page.adoc,字>>` は
            // 別の文書への参照として読まれないためです(2026-09-02 に本家の
            // 手引き 176 枚で退行して分かった)
            if f.name.contains(['/', '.', '#', ':', '{']) {
                let text = if r.text == f.name { String::new() } else { esc(&r.text) };
                s.push_str(&format!("xref:{}[{}]", esc(&f.name), text));
            } else if r.text.is_empty() || r.text == f.name {
                s.push_str(&format!("<<{}>>", esc(&f.name)));
            } else {
                s.push_str(&format!("<<{},{}>>", esc(&f.name), esc(&r.text)));
            }
            continue;
        }
        // 上付き・下付きは**意味**(x² / H₂O)。AsciiDoc の標準の印。
        // 中に空白があるときは `^+字+^` と passthrough で包みます(本家は
        // 上付きの中に空白を許さない)
        let (sup, sub) = (r.fmt.superscript, r.fmt.subscript);
        let padded = r.text.chars().any(char::is_whitespace);
        let (sup_open, sup_close) = match (sup, sub, padded) {
            (true, _, true) => ("^+", "+^"),
            (true, _, false) => ("^", "^"),
            (_, true, true) => ("~+", "+~"),
            (_, true, false) => ("~", "~"),
            _ => ("", ""),
        };
        s.push_str(sup_open);
        // **文字単位のスタイル**(2026-08-16)は AsciiDoc の役割の書き方
        // `[.名前]#字#`。蛍光ペンは本家のマーク `#字#`(色は運びません)。
        // 前後が字か、中身の端が空白なら二重の印
        let before = s.chars().last();
        let after = runs.get(ri + 1).and_then(first_char);
        let dbl = is_char(before)
            || is_char(after)
            || is_space(r.text.chars().next())
            || is_space(r.text.chars().last());
        let (open, close) = match role {
            Some(_) if role_on_mark.is_some() => (String::new(), ""),
            // リンクの役割は `[字,role=名前]` に入れる(下で書く)
            Some(_) if r.fmt.link.is_some() => (String::new(), ""),
            Some(n) if dbl => (format!("[.{n}]##"), "##"),
            Some(n) => (format!("[.{n}]#"), "#"),
            None if r.fmt.highlight.is_some() && dbl => ("##".to_string(), "##"),
            None if r.fmt.highlight.is_some() => ("#".to_string(), "#"),
            None => (String::new(), ""),
        };
        s.push_str(&open);
        if let Some(sdt) = &r.fmt.sdt {
            s.push_str(&field_src(sdt));
        } else if let Some(ruby) = &r.fmt.ruby {
            s.push_str(&format!("ruby:{}[{}]", esc(&r.text), ruby));
        } else if let Some(url) = &r.fmt.link {
            // **前が字なら `link:` を付けます。** 本家は `表計算https://…[名]` を
            // リンクとして読みません(2026-08-18 に本家で確かめました)。
            // 裸の URL は、直前が行頭・空白・`<>()[];"'` のときだけリンクに
            // なるので、それ以外は `link:` を付けます
            let prev = s.chars().last();
            // 直前が `<` なら裸で書かない(`<URL>` の形に読まれて `<` が消える)
            let bare_ok = prev.map_or(true, |c| {
                c.is_whitespace() || matches!(c, '>' | '(' | ')' | '[' | ']' | ';' | '"' | '\'')
            });
            // 裸の URL は後ろの字を飲み込むので、次が空白か `,.?!);:` のときだけ裸で書く
            let next = runs.get(ri + 1).and_then(first_char);
            let next_ok = next.map_or(true, |c| c.is_whitespace() || matches!(c, ',' | '.' | '?' | '!' | ')' | ';' | ':'));
            let url_tail_ok = !url.ends_with([',', '.', '?', '!', ')', ';', ':']);
            let head = if prev.is_some_and(|c| c.is_alphanumeric()) { "link:" } else { "" };
            // 対象が空なら `link:[字]` の形しかありません
            let head = if url.is_empty() { "link:" } else { head };
            if r.text == *url && role.is_none() {
                // 字と URL が同じなら裸で書く(本家の `class="bare"` と同じ)
                if bare_ok && next_ok && url_tail_ok && !url.is_empty() && !url.contains(['[', ']']) {
                    s.push_str(url);
                } else {
                    s.push_str(&format!("{head}{url}[]"));
                }
            } else {
                // `]` は `\]` に逃がし、`\` は passthrough で包む(`\]` と紛れないように)
                let text = if r.text == *url {
                    String::new()
                } else {
                    esc(&r.text.replace('\\', "+\\+").replace(']', "\\]"))
                };
                let text = if text.contains('=') || (role.is_some() && text.contains(',')) {
                    format!("\"{text}\"")
                } else {
                    text
                };
                match role {
                    Some(n) if text.is_empty() => s.push_str(&format!("{head}{url}[role={n}]")),
                    Some(n) => s.push_str(&format!("{head}{url}[{text},role={n}]")),
                    None => s.push_str(&format!("{head}{url}[{text}]")),
                }
            }
        } else {
            s.push_str(&esc(&r.text));
        }
        s.push_str(close);
        s.push_str(sup_close);
    }
    while let Some((k, dbl)) = stack.pop() {
        s.push_str(mark_of(k, dbl));
    }
    // **書いた行を読み直して、意図と違う所だけ `\` を足します。**
    // 本家の `\` は「次の印は字です」の意味で、印として読まれない所に
    // 付けると `\` がそのまま読者に見えます(2026-09-02)
    crate::inline::settle(&s, runs)
}

/// 本文の字の中の、印にもなりうる字に仮の字を付ける。
/// 本物に戻して `\` を足すかどうか決めるのは [`crate::inline::settle`]
fn esc(t: &str) -> String {
    t.chars().map(crate::inline::twin).collect()
}

/// 記入欄の `[…]` の中身 → (表示名, 種類, 選択肢)。
///
/// 種類の名前は日本語で書きます。知らない名前は**文字の欄として扱い、
/// 黙って捨てません**(表示名の一部として残ります)。
pub(crate) fn parse_field(s: &str) -> (String, crate::doc::SdtKind, Vec<String>) {
    use crate::doc::SdtKind as K;
    let mut items = Vec::new();
    let (alias, rest) = match s.split_once(',') {
        Some((a, r)) => (a.trim().to_string(), r.trim()),
        None => (s.trim().to_string(), ""),
    };
    if let Some(list) = rest.strip_prefix("選ぶ:").or_else(|| rest.strip_prefix("打てる選ぶ:")) {
        items = list.split('|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        let k = if rest.starts_with("打てる選ぶ") { K::Combo } else { K::Dropdown };
        return (alias, k, items);
    }
    let kind = match rest {
        "複数行" => K::Complex,
        "チェック" => K::Checkbox,
        "ラジオ" => K::Radio,
        "日付" => K::Date,
        "メール" => K::Email,
        "電話" => K::Phone,
        "画像" => K::Picture,
        "署名" => K::Signature,
        _ => K::Text,
    };
    // 知らない種類は表示名に戻します(黙って落とさない)
    let alias = if kind == K::Text && !rest.is_empty() && rest != "文字" {
        format!("{alias},{rest}")
    } else {
        alias
    };
    (alias, kind, items)
}

/// 記入欄 → `field:…[…]` の字。読み手と対になります。
fn field_src(s: &crate::doc::Sdt) -> String {
    use crate::doc::SdtKind as K;
    let mut o = format!("field:{}[{}", s.tag, s.alias);
    match s.kind {
        K::Text => {}
        K::Dropdown => o.push_str(&format!(",選ぶ:{}", s.items.join("|"))),
        K::Combo => o.push_str(&format!(",打てる選ぶ:{}", s.items.join("|"))),
        K::Complex => o.push_str(",複数行"),
        K::Checkbox => o.push_str(",チェック"),
        K::Radio => o.push_str(",ラジオ"),
        K::Date => o.push_str(",日付"),
        K::Email => o.push_str(",メール"),
        K::Phone => o.push_str(",電話"),
        K::Picture => o.push_str(",画像"),
        K::Signature => o.push_str(",署名"),
    }
    o.push(']');
    o
}

/// 表の中の空行の印。`a|`(AsciiDoc として組むセル)の中では**段落の
/// 切れ目**になります。本文に出ない字なので、中身と紛れません。
const TABLE_BLANK_ROW: &str = "\u{0}";

/// 空の段落の書き方(本家の作法)。空行を並べても1つの切れ目にまとまるので、
/// **何行あったか**を残すにはこれを置きます。
const EMPTY_PARA: &str = "{empty}";

/// **このセルの字は式か。**
///
/// 表計算の式(`=SUM(B2:B4)`)は、セルの中の書き方(太字・斜体)として
/// 読んではいけません。`=A2*B2*C2` の `*B2*` を太字と読むと、印が消えて
/// `A2B2C2` という別の式になり、**黙って `#NAME?` に化けます**
/// (2026-08-19 に実際に踏みました)。
///
/// セルの中の見出し(`= 見出し` `== 見出し`)と見分けるのは**空白**です。
/// 式は `=` の後ろに空白を置きません。
///
/// 読みと書きの両方がこの1つの決めを見ます — 2箇所に書くと必ずずれます。
pub use book::is_formula_cell;

/// そのセルの格子の列(左のセルの span の和)
pub(crate) fn grid_col(row: &[Cellbox], k: usize) -> usize {
    row[..k].iter().map(|c| c.span()).sum()
}

/// 縦結合の始まりが呑む行数(自分+下の Continue の数)。
///
/// **HTML の書き出しも同じ数え方を使います**([`crate::html_write`])。
/// 結合の数え方が2箇所にあると、adoc と HTML で表の形が違ってきます。
pub(crate) fn vspan_of(t: &Table, ri: usize, col: usize) -> u8 {
    let mut n = 1u8;
    for row in &t.rows[ri + 1..] {
        let hit = row
            .iter()
            .enumerate()
            .find(|(k, _)| grid_col(row, *k) == col)
            .map(|(_, c)| c.v_merge == VMerge::Continue)
            .unwrap_or(false);
        if hit {
            n += 1;
        } else {
            break;
        }
    }
    n
}

fn write_table(out: &mut String, t: &Table, doc: &Document) {
    // **表の役割**(`[.names]`)。題より前に書く — AsciiDoc は塊の指定を
    // 題の前に置く作法です
    if let Some(role) = &t.role {
        out.push_str(&format!("[.{role}]\n"));
    }
    // 表の題(`.名前`)。表の名前になるので、桁の指定より前に書く
    if let Some(name) = &t.title {
        out.push_str(&format!(".{name}\n"));
    }
    // **`a|` のセルがあるときは桁の数を必ず書きます。** 中身が次の行に
    // 続くので、読むときに「最初の行のセルの数」では桁を数えられません
    // **改行を持つセルも「複数行のセル」です**(2026-08-31)。`a|` で書かれ、
    // 中身が次の行へ続くので、桁の数を `[cols=]` で先に言っておかないと
    // 読む側が行の切れ目を見誤ります(隣のセルが次の行へ落ちました)
    let has_many_paras = t.rows.iter().flatten().any(|c| {
        c.paragraphs.len() > 1
            || c.paragraphs.iter().any(|p| p.runs.iter().any(|r| r.text.contains('\n')))
    });
    if t.col_ratio.is_empty() && has_many_paras {
        let cols: usize = t.rows.first().map(|r| r.iter().map(|c| c.span()).sum()).unwrap_or(0);
        if cols > 0 {
            out.push_str(&format!("[cols=\"{}\"]\n", vec!["1"; cols].join(",")));
        }
    }
    // 桁の割合(`[cols="1,3"]`)。表の直前の行として書く
    if !t.col_ratio.is_empty() {
        let numbers: Vec<String> = t
            .col_ratio
            .iter()
            .map(|v| {
                if (v - v.round()).abs() < 0.001 {
                    format!("{}", *v as i64)
                } else {
                    format!("{v}")
                }
            })
            .collect();
        out.push_str(&format!("[cols=\"{}\"]\n", numbers.join(",")));
    }
    out.push_str("|===\n");
    for (ri, row) in t.rows.iter().enumerate() {
        // この行にもうセルを書いたか(縦の結合の続きは書かないので、
        // 番号ではなく実際に書いたかで見ます)
        let mut wrote = false;
        // 直前のセルが複数段落だったか(次のセルを行頭から始めるため)
        let mut prev_is_multi = false;
        for (k, cell) in row.iter().enumerate() {
            // 縦結合の続き = セルを書かない(頭の .N+ が占める)
            if matches!(cell.v_merge, VMerge::Continue) {
                continue;
            }
            // **セルの間に空白を1つ置きます**(2026-08-18)。詰めて書くと、
            // 前のセルの終わりの `8*` が「8回くり返す」の指定として読まれ、
            // 表が崩れます(本家で確かめました)。
            //
            // **区切りは結合の頭書きより先に書きます**(2026-08-19 に直した)。
            // 逆にすると `6+` が前のセルの字の末尾にくっつき、区切りの空白が
            // その後ろに来るので、読むときに指定として読めません。行の
            // 2つ目から先の結合が**黙って消えて**いました
            // **セルの中に改行があるときも `a|` にします**(2026-08-31)。
            // 素のセルは1行しか書けないので、改行がそのまま出ると行が割れます
            let ori_ari = cell
                .paragraphs
                .iter()
                .any(|p| p.runs.iter().any(|r| r.text.contains('\n')));
            let many_paras = cell.paragraphs.len() > 1 || ori_ari;
            if wrote {
                // 前のセルが複数段落なら、行を変えて次のセルを始めます
                out.push(if prev_is_multi { '\n' } else { ' ' });
            }
            wrote = true;
            prev_is_multi = many_paras;
            if let VMerge::Start = cell.v_merge {
                let n = vspan_of(t, ri, grid_col(row, k));
                if n > 1 {
                    out.push_str(&format!(".{n}+"));
                }
            }
            if cell.span() > 1 {
                out.push_str(&format!("{}+", cell.span()));
            }
            // **段落が2つ以上のセルは `a|`** にします(本家の作法)。
            // 素のセルは中身を1段落として組むので、詰めて書くと段落の
            // 切れ目が消えます(実物の様式で 63 セルが当たりました)
            if many_paras {
                out.push('a');
            }
            out.push('|');
            // **式は字のまま出します。** `=C2*150` の `*` を逃がすと
            // `=C2\*150` になり、設計の見本とも本家の見え方とも違います
            let raw: String = cell
                .paragraphs
                .iter()
                .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join(" ");
            let text = if is_formula_cell(&raw) {
                raw
            } else {
                let mut paras = cell.paragraphs.iter().map(|p| runs_text(&p.runs, doc)).collect::<Vec<_>>();
                // **空の段落は `{empty}` で書きます**(本家の書き方)。
                // 空行を並べても本家は1つの切れ目にまとめるので、様式の
                // 「書き込む余白」が何行あったかが消えてしまいます
                if many_paras {
                    for p in paras.iter_mut() {
                        if p.trim().is_empty() {
                            *p = EMPTY_PARA.to_string();
                        }
                    }
                }
                // 複数段落は空行で区切ります(`a|` の中身の作法)。
                // **段落の中の改行は AsciiDoc の改行**(行末の ` +`)です。
                // 空行で区切ると段落が2つになり、間が余分に空きます —
                // 表計算のセルの「区　分 / Type」はそれでは違う見え方です
                let mut t = paras
                    .join(if many_paras { "\n\n" } else { " " })
                    .replace('\n', " +\n")
                    .replace(" +\n +\n", "\n\n");
                // **継ぐ行の頭の空白を守ります**(2026-08-31)。読む側は
                // 続きの行の頭を落とします(adoc の字下げは字ではないため)。
                // `{empty}` を置くと、その後ろの空白は字として残ります
                // 1行目の頭も同じです。`|` のすぐ後ろの空白は、読む側が
                // セルの区切りの飾りとして落とします
                // 行末の空白も同じです。`|` の手前の空白はセルの区切りの
                // 飾りなので、読む側が落とします
                t = t
                    .split('\n')
                    .map(|l| {
                        let mut o = l.to_string();
                        if o.starts_with(' ') {
                            o.insert_str(0, EMPTY_PARA);
                        }
                        if o.ends_with(' ') {
                            o.push_str(EMPTY_PARA);
                        }
                        o
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                t
            };
            // **縦棒は逃がします**(2026-08-20 に見つけた)。逃がさないと
            // 中身の `|` が次のセルの頭と読まれ、**1つのセルが2つに割れて
            // 行がずれます**。`|===` を含むセルなら表そのものが途中で
            // 終わります。読む側は前から `\|` を飛ばしていたので、
            // 足りなかったのは書く側だけです。
            //
            // *式も逃がします。* `="A|B"` のようなセルがあるためです。
            // `*` を逃がさない決め(下の `is_formula_cell` の枝)とは別で、
            // `|` はセルの切れ目そのものなので、逃がさないと形が壊れます
            out.push_str(&text.replace('|', "\\|"));
        }
        out.push('\n');
        if ri == 0 && t.header_row {
            out.push('\n'); // 見出しの行の印
        }
    }
    out.push_str("|===\n\n");
}

/// 画像の中身から拡張子を見ます(先頭の数バイトで分かります)。
///
/// **HTML の書き出しも同じ物を使います**([`crate::html_write`])。名前の付け方が
/// 2箇所にあると、同じ画像が形式によって違う名前になります。
pub fn image_ext(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "jpg",
        [b'G', b'I', b'F', ..] => "gif",
        [b'R', b'I', b'F', b'F', ..] => "webp",
        _ => "png",
    }
}

/// **径路の無い画像に径路を与えます。** 返りは、本文と一緒に書き出す
/// ファイル(本文から見た相対の径路, 中身)。
///
/// adoc は画像を `image::径路[]` で指すので、径路が無い画像は書けません。
/// docx から来た画像や、画面から挿した画像は径路を持っていないので、ここで
/// 名前を付けます。**付けないと保存で絵が消えます**(2026-08-18 に直した)。
///
/// 画像の実体を書くのはこの関数の仕事ではありません(engine はファイルを
/// 触りません)。呼ぶ側が返りをファイルに書きます。
pub fn assign_image_paths(doc: &mut Document) -> Vec<(String, std::sync::Arc<Vec<u8>>)> {
    // すでに使われている名前(同じ名前で上書きしないため)
    let mut used: Vec<String> = Vec::new();
    each_image(doc, &mut |im| {
        if let Some(s) = &im.src {
            used.push(s.clone());
        }
    });
    let mut out = Vec::new();
    let mut n = 0usize;
    each_image(doc, &mut |im| {
        if im.src.is_some() || im.bytes.is_empty() {
            return;
        }
        let name = loop {
            n += 1;
            let name = format!("images/図{n}.{}", image_ext(&im.bytes));
            if !used.contains(&name) {
                break name;
            }
        };
        used.push(name.clone());
        im.src = Some(name.clone());
        out.push((name, im.bytes.clone()));
    });
    out
}

/// 文書の中の画像を、出てくる順に1つずつ渡します(表の中も見ます)。
fn each_image(doc: &mut Document, f: &mut dyn FnMut(&mut InlineImage)) {
    for b in &mut doc.blocks {
        let paras: Vec<&mut Paragraph> = match b {
            Block::Para(p) => vec![p],
            Block::Table(t) => t
                .rows
                .iter_mut()
                .flat_map(|r| r.iter_mut())
                .flat_map(|c| c.paragraphs.iter_mut())
                .collect(),
        };
        for p in paras {
            for im in p.images_new.iter_mut().chain(p.images.iter_mut()) {
                f(im);
            }
        }
    }
}

/// **この文書を adoc で保存すると落ちるもの。**
///
/// adoc は意味だけを持つので、見た目とページの飾りは保存で消えます。
/// 消すこと自体は決めたとおりですが、**黙って消しません** — 何が消えるかを
/// 数えて呼ぶ側に返し、呼ぶ側が人に見せます。
///
/// 消えた物の行き先はテンプレートです(SEKKEI の対応表)。ただしテンプレートが
/// まだ持てない欄(ヘッダー・フッター・透かし・縦書き)もあるので、いまは
/// 「消える」と言うのが正確です。
pub fn dropped(doc: &Document) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    let mut push = |name: &'static str| {
        if !v.contains(&name) {
            v.push(name);
        }
    };
    if !doc.header.paragraphs.is_empty() {
        push("ヘッダー");
    }
    if !doc.footer.paragraphs.is_empty() {
        push("フッター");
    }
    if doc.watermark.is_some() {
        push("透かし");
    }
    if doc.page_color.is_some() {
        push("ページの色");
    }
    if doc.vertical {
        push("縦書き");
    }
    if doc.page.map(|p| p.columns > 1).unwrap_or(false) {
        push("段組み");
    }
    // 手描きの線は保存で SVG の絵になります(writer の `ink_to_images`)。
    // 消えないので、ここでは数えません
    // **表の中の段落も見ます。** 事務の様式は中身が表の中にあるので、
    // 本文だけ見ると「何も落ちません」と嘘を言うことになります
    let in_table = doc.blocks.iter().filter_map(|b| match b {
        Block::Table(t) => Some(t),
        Block::Para(_) => None,
    });
    let cells = in_table
        .flat_map(|t| t.rows.iter())
        .flat_map(|r| r.iter())
        .flat_map(|c| c.paragraphs.iter());
    for p in doc.paragraphs().chain(cells) {
        if !p.comments.is_empty() {
            push("コメント");
        }
        if p.align != crate::doc::Align::Left {
            push("段落の揃え");
        }
        if p.indent > 0 || p.first_line_twips != 0 {
            push("字下げ");
        }
        if p.line_spacing > 0.0 && p.line_spacing != 1.0 {
            push("行間");
        }
        if p.space_before_pt != 0.0 || p.space_after_pt != 0.0 {
            push("段落の前後の空き");
        }
        if p.shade.is_some() {
            push("段落の背景色");
        }
        if p.boxed {
            push("段落の囲み");
        }
        if p.dropcap {
            push("ドロップキャップ");
        }
        if matches!(p.style, ParaStyle::Toc(_) | ParaStyle::Tof) {
            push("目次の印");
        }
        for r in &p.runs {
            // 相互参照は「しおりの文字」と「ページ番号」の2種類だが、adoc は
            // `<<名前>>` の1種類しか持たない。ページ番号は文字の参照になる
            if r.fmt.field.as_ref().is_some_and(|f| f.page) {
                push("相互参照のページ番号");
            }
            if r.fmt.underline {
                push("下線");
            }
            if r.fmt.strike {
                push("取り消し線");
            }
            if r.fmt.color.is_some() {
                push("文字の色");
            }
            if r.size_pt.is_some() {
                push("字の大きさ");
            }
            if r.font.is_some() {
                push("フォント");
            }
        }
    }
    v
}

// ---- 読み ------------------------------------------------------------------

/// **本家の AsciiDoc にはあるが、うちが扱わない書き方。** 見つけたら名前を返す。
///
/// 字は本文として残しますが、意味は落ちています。落ちたことを言うためだけの
/// 判定なので、**確かに本家の書き方だと分かる形だけ**を見ます(迷う形は
/// 見ません — 普通の日本語の文を「読めなかった」と言うほうが害が大きい)。
/// 次の行が空行か(原文のまま持ち越すとき、後ろの空行も含めるため)
fn next_is_blank<'a, I: Iterator<Item = (usize, &'a str)>>(
    it: &mut std::iter::Peekable<I>,
) -> bool {
    it.peek().map(|(_, l)| l.trim().is_empty()).unwrap_or(false)
}

// ---- 節の用紙(asciidoctor-pdf の書き方)---------------------------------

/// 節の用紙の属性の名前。`[page-size=A3, page-layout=landscape, page-margin=15mm]`
/// を改ページ `<<<` の前に置くと、そこから新しい節になり、用紙が変わります
/// (asciidoctor-pdf の `page-layout` と同じ形。size と margin はその仲間です)。
/// 頭の `:page-size:` などは最初の節の用紙です。
const PAGE_ATTR_KEYS: &[&str] = &["page-size", "page-layout", "page-margin", "page-columns"];

fn is_page_attr_key(k: &str) -> bool {
    PAGE_ATTR_KEYS.contains(&k)
}

/// `[k=v, k=v]` の行なら、対を返します。**page- の名前を1つも持たない行は
/// 節の用紙ではありません**(`[source,python]` などは今までどおり)。
fn page_attrs(l: &str) -> Option<Vec<(String, String)>> {
    let inner = l.strip_prefix('[')?.strip_suffix(']')?;
    if inner.starts_with('[') || inner.starts_with('.') {
        return None;
    }
    // 角括弧の中のコンマ(`page-margin=[20, 15, 20, 15]`)では切りません
    let mut parts: Vec<String> = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in inner.chars() {
        match ch {
            '[' => { depth += 1; cur.push(ch); }
            ']' => { depth -= 1; cur.push(ch); }
            ',' if depth == 0 => { parts.push(std::mem::take(&mut cur)); }
            _ => cur.push(ch),
        }
    }
    parts.push(cur);
    let mut out = Vec::new();
    for part in parts {
        let (k, v) = part.trim().split_once('=')?;
        out.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
    }
    if out.iter().any(|(k, _)| is_page_attr_key(k)) { Some(out) } else { None }
}

/// mm の数を読む。`20` も `20mm` も 20。
fn mm_of(v: &str) -> Result<f32, String> {
    v.trim().trim_end_matches("mm").trim().parse::<f32>().map_err(|_| format!("mm の数で書いてください: {v}"))
}

/// 用紙の属性を、いまの用紙に重ねます。
fn apply_page_attrs(base: crate::PageSetup, attrs: &[(String, String)]) -> Result<crate::PageSetup, String> {
    let mut pg = base;
    for (k, v) in attrs {
        match k.as_str() {
            "page-size" => {
                let landscape = pg.w_mm > pg.h_mm;
                let (w, h) = crate::theme::youshi_mm(v)
                    .or_else(|| crate::theme::youshi_mm(&v.to_ascii_uppercase()))
                    .ok_or_else(|| format!("知らない用紙: {v}(A4 / A3 / B5 / Letter)"))?;
                let (w, h) = (w.min(h), w.max(h));
                (pg.w_mm, pg.h_mm) = if landscape { (h, w) } else { (w, h) };
            }
            "page-layout" => {
                let landscape = match v.to_ascii_lowercase().as_str() {
                    "landscape" | "横" => true,
                    "portrait" | "縦" => false,
                    other => return Err(format!("page-layout は portrait か landscape: {other}")),
                };
                let (w, h) = (pg.w_mm.min(pg.h_mm), pg.w_mm.max(pg.h_mm));
                (pg.w_mm, pg.h_mm) = if landscape { (h, w) } else { (w, h) };
            }
            "page-margin" => {
                let t = v.trim();
                if let Some(list) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    let ns: Vec<f32> = list.split(',').map(mm_of).collect::<Result<_, _>>()?;
                    match ns.as_slice() {
                        [a] => { pg.top_mm = *a; pg.right_mm = *a; pg.bottom_mm = *a; pg.left_mm = *a; }
                        [tb, lr] => { pg.top_mm = *tb; pg.bottom_mm = *tb; pg.left_mm = *lr; pg.right_mm = *lr; }
                        [t, r, b, l] => { pg.top_mm = *t; pg.right_mm = *r; pg.bottom_mm = *b; pg.left_mm = *l; }
                        _ => return Err("page-margin は 1・2・4 つの数(上, 右, 下, 左)で書きます".into()),
                    }
                } else {
                    let m = mm_of(t)?;
                    (pg.top_mm, pg.right_mm, pg.bottom_mm, pg.left_mm) = (m, m, m, m);
                }
            }
            "page-columns" => {
                pg.columns = v.trim().parse::<u8>().map_err(|_| format!("page-columns は段の数: {v}"))?;
            }
            _ => {} // page- でない属性は、ここでは読みません
        }
    }
    Ok(pg)
}

/// 頭の `:page-size:` などから最初の節の用紙を作ります。無ければ None
fn page_from_attrs(attrs: &[(String, String)]) -> Result<Option<crate::PageSetup>, String> {
    let pairs: Vec<(String, String)> = attrs.iter().filter(|(k, _)| is_page_attr_key(k)).cloned().collect();
    if pairs.is_empty() {
        return Ok(None);
    }
    apply_page_attrs(crate::PageSetup::default(), &pairs).map(Some)
}

/// 用紙の名前(英語)。読む側は日本語の名前も受けます
fn youshi_en(pg: &crate::PageSetup) -> &'static str {
    match crate::theme::youshi_na((pg.w_mm.min(pg.h_mm), pg.w_mm.max(pg.h_mm))) {
        "レター" => "Letter",
        na => na,
    }
}

fn mm_text(v: f32) -> String {
    if (v - v.round()).abs() < 0.01 { format!("{}", v.round() as i64) } else { format!("{v:.1}") }
}

/// 用紙を属性の対に(書く側)。size・layout・margin は必ず、columns は 2 段以上のとき
fn page_attr_pairs(pg: &crate::PageSetup) -> Vec<(&'static str, String)> {
    let margin = if pg.top_mm == pg.right_mm && pg.top_mm == pg.bottom_mm && pg.top_mm == pg.left_mm {
        format!("{}mm", mm_text(pg.top_mm))
    } else {
        format!("[{}mm, {}mm, {}mm, {}mm]", mm_text(pg.top_mm), mm_text(pg.right_mm), mm_text(pg.bottom_mm), mm_text(pg.left_mm))
    };
    let mut v = vec![
        ("page-size", youshi_en(pg).to_string()),
        ("page-layout", if pg.w_mm > pg.h_mm { "landscape" } else { "portrait" }.to_string()),
        ("page-margin", margin),
    ];
    if pg.cols() > 1 {
        v.push(("page-columns", pg.cols().to_string()));
    }
    v
}

/// `[page-size=A4, page-layout=portrait, page-margin=20mm]` の行(書く側)
fn page_attr_line(pg: &crate::PageSetup) -> String {
    let body: Vec<String> = page_attr_pairs(pg).into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("[{}]", body.join(", "))
}


fn vendor_only_syntax(l: &str) -> Option<(&'static str, &'static str)> {
    let t = l.trim_end();
    let ts = t.trim_start();
    // 塊の区切り(DELIMITED_BLOCKS)。**うちが意味を知っている物は除く** —
    // 引用(____)と表(|===)は編集できます
    for (mark, name) in DELIMITED {
        if is_delim(t, mark) {
            return Some((name, "塊の区切り"));
        }
    }
    // 横の区切り線。改ページ(<<<)はうちが扱うので除く
    if matches!(t, "'''" | "---" | "***" | "___") {
        return Some(("横の区切り線", "横の区切り線"));
    }
    if ts.starts_with("include::") {
        return Some(("取り込み(include::)", "取り込み"));
    }
    // 条件の行(`ifdef::属性[]` `ifndef::` `ifeval::` `endif::`)
    if ["ifdef::", "ifndef::", "ifeval::", "endif::"].iter().any(|h| ts.starts_with(h)) {
        return Some(("条件の行(ifdef::)", "条件"));
    }
    // 本文の途中の属性の項目(`:名前: 値`)。頭の外でも本家は受けます
    if let Some(rest) = ts.strip_prefix(':') {
        if let Some((k, _)) = rest.split_once(':') {
            let k = k.strip_suffix('!').unwrap_or(k);
            if !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                return Some(("属性の項目(:名前: 値)", "属性の項目"));
            }
        }
    }
    // **作業のリスト**(`* [ ] やること`)。**段も見ます** —
    // 2026-08-25 まで1段目しか拾わず、`** [ ]` は普通の箇条書きになって
    // `[ ]` が字として残っていました。`- [ ]`(Markdown の書き方)も
    // `[x]` だけ拾って `[ ]` を落としていました
    if is_task_list(ts) {
        return Some(("チェックの箇条書き", "チェック"));
    }
    if ts.starts_with("//") {
        return Some(("覚え書きの行(//)", "覚え書き"));
    }
    // コールアウトの一覧(`<1> 説明`)。コードの塊の後ろに並ぶ。行ごとに原文のまま
    if ts.starts_with('<') {
        if let Some(rest) = ts.strip_prefix('<').and_then(|r| r.split_once('>')) {
            let (num, after) = rest;
            if !num.is_empty() && (num == "." || num.chars().all(|c| c.is_ascii_digit())) && after.starts_with(' ') {
                return Some(("コールアウトの一覧(<1>)", "コールアウト"));
            }
        }
    }
    // 塊の題(.題)。箇条書きの `. ` とは違う
    if ts.starts_with('.') && !ts.starts_with(". ") && ts.len() > 1 && !ts.starts_with("..") {
        return Some(("塊の題(.題)", "塊の題"));
    }
    // **塊の指定の行**(`[source,python]` `[cols="1,3"]` `[quote, 誰]` など)。
    // 次の塊に掛かる行なので、**続きの行と離してはいけません**。前は普通の
    // 段落として読んでいたので、保存で `[source,python]` と `----` の間に
    // 空行が入り、指定が塊に掛からなくなっていました(2026-08-18 に
    // 実際に往復させて見つけました)。
    //
    // **`[[しおり]]` と `[.スタイル名]` は除きます** — こちらが意味を
    // 知っている書き方で、この関数より後ろで読まれます(2026-08-18 に
    // しおりを飲み込んで試験が落ちました)
    if ts.starts_with('[')
        && ts.ends_with(']')
        && ts.len() > 2
        && !ts.starts_with("[[")
        && !ts.starts_with("[.")
    {
        return Some(("塊の指定([…])", "指定の行"));
    }
    // 属性の参照 {member}。うちの差し込みは {{member}} なので、二重は数えない
    let b = ts.as_bytes();
    for (i, c) in ts.char_indices() {
        let double = b.get(i + 1) == Some(&b'{') || (i > 0 && b[i - 1] == b'{');
        if c == '{' && !double && ts[i..].contains('}') {
            return Some(("属性の参照({member})", ""));
        }
    }
    None
}

/// **等幅の字のスタイル名。** 読む側と書く側で同じ名前を使うための1箇所
pub const MONO: &str = "等幅";

/// 註記の頭(asciidoctor の `ADMONITION_STYLES`)
const ADMONITION: &[&str] = &["NOTE:", "TIP:", "IMPORTANT:", "WARNING:", "CAUTION:"];

/// 註記の段落のスタイル名。**並びは [`ADMONITION`] と同じ**(印ごとに別の
/// スタイルにするので、テンプレートで色を分けられます)。2026-08-18。
///
/// どれなのかを字に残さないので、本文を直しても印は壊れません
const ADMON_STYLE: &[&str] = &["註記", "ヒント", "重要", "警告", "注意"];

/// **塊の中身の段落のスタイル名。** 種類ごとに別の名前にして、テンプレートで
/// 見た目(等幅・背景・出さない)を決められるようにします(2026-09-03)。
/// 書くときはどれも字のまま1行で返します(印は付けません)
const BLOCK_INNER: &[&str] = &[
    "塊の中", "コードの塊", "字のまま出す塊", "例の塊", "傍注の塊", "覚え書きの塊",
    "そのまま通す塊", "詩の塊",
    "註記の塊", "ヒントの塊", "重要の塊", "警告の塊", "注意の塊",
];

/// 塊の中身の段落か(書くときは字のまま返す)
pub fn is_block_inner(name: &str) -> bool {
    BLOCK_INNER.contains(&name)
}

/// 画面と紙で**等幅**にする塊
pub fn is_mono_block(name: &str) -> bool {
    matches!(name, "塊の中" | "コードの塊" | "字のまま出す塊" | "そのまま通す塊" | "字下げ")
}

/// 画面と紙に**出さない**行(覚え書き)
pub fn is_hidden_block(name: &str) -> bool {
    matches!(name, "覚え書きの塊" | "覚え書き")
}

/// 塊の中身のスタイル名を、区切りの印と直前の指定の行(`[source,python]`
/// `[NOTE]` `[verse]` など)から決めます。
fn block_inner_style(mark: &str, spec: Option<&str>) -> &'static str {
    let spec = spec.map(|s| s.trim().trim_start_matches('[').trim_end_matches(']').to_string());
    let first = spec.as_deref().and_then(|s| s.split(',').next()).map(|s| s.trim().to_string());
    if let Some(f) = first.as_deref() {
        if let Some(i) = ADMONITION.iter().position(|m| m.trim_end_matches(':') == f) {
            return ["註記の塊", "ヒントの塊", "重要の塊", "警告の塊", "注意の塊"][i];
        }
        match f {
            "verse" => return "詩の塊",
            "literal" => return "字のまま出す塊",
            "source" | "listing" => return "コードの塊",
            _ => {}
        }
    }
    match mark.chars().next().unwrap_or(' ') {
        '-' if mark.trim() == "--" => "塊の中",
        '-' | '`' => "コードの塊",
        '.' => "字のまま出す塊",
        '=' => "例の塊",
        '*' => "傍注の塊",
        '/' => "覚え書きの塊",
        '+' => "そのまま通す塊",
        _ => "塊の中",
    }
}

/// **1行で1つと決まっている段落のスタイル名か。**
/// 註記とラベル付きリストは、続く行を呑むと形が壊れます
fn one_per_line(name: &str) -> bool {
    is_desc_list(name) || admon_mark(name).is_some()
}

/// **ラベル付きリストの行か**(始めの行も含む)。
///
/// 空行で区切られた2つ目の一覧は [`説明のリストの始め`] という名前です。
/// 印が無いと、書き戻しで空行が消えて*2つの一覧が1つに繋がります*
/// (2026-08-25。問答形式を見ていて見つけました)
pub(crate) fn is_desc_list(name: &str) -> bool {
    name == "説明のリスト" || name == DESC_LIST_START
}

/// 空行のあとに始まるラベル付きリストの印
pub(crate) const DESC_LIST_START: &str = "説明のリストの始め";

/// 箇条書きの行か。返るのは(段, 中身)。段は 0 から。
/// AsciiDoc は印の数が段です(`*` が1段目、`**` が2段目)
fn is_bullet(l: &str, mark: char) -> Option<(u8, &str)> {
    let n = l.chars().take_while(|c| *c == mark).count();
    // 本家は印の数に上限を置かない(試験には 26 段の物がある)
    if n == 0 || n > 40 {
        return None;
    }
    let rest = l[n..].strip_prefix(' ')?;
    Some(((n - 1) as u8, rest))
}

/// 番号を明示した箇条書き(`7. 字` `a. 字` `i) 字` `IV) 字`)。返るのは(印, 中身)。
/// 本家はこれらを番号付きの一覧として読む(印が番号の始めと種類を決める)
fn is_numbered_marker(l: &str) -> Option<(&str, &str)> {
    let (mark, rest) = l.split_once(' ')?;
    if mark.is_empty() || rest.is_empty() {
        return None;
    }
    let body = mark.strip_suffix('.').or_else(|| mark.strip_suffix(')'))?;
    if body.is_empty() {
        return None;
    }
    let digits = body.chars().all(|c| c.is_ascii_digit());
    let alpha = body.chars().count() == 1 && body.chars().all(|c| c.is_ascii_alphabetic());
    let roman = mark.ends_with(')') && body.chars().all(|c| "ivxlcdmIVXLCDM".contains(c));
    if (digits && mark.ends_with('.')) || alpha || roman {
        Some((mark, rest))
    } else {
        None
    }
}

/// 箇条書きの印として、書くときにそのまま返せる物か(docx から来た `○` などは違う)
fn is_adoc_list_mark(m: &str) -> bool {
    m == "-" || is_numbered_marker(&format!("{m} x")).is_some()
}

/// `-` の箇条書きの行か。返るのは中身。
///
/// **`-` は1段目だけ**です(AsciiDoc の決め。`--` は塊の囲みなので、
/// 箇条書きとして読むと囲みが壊れます)。
fn is_dash_bullet(l: &str) -> Option<&str> {
    let rest = l.strip_prefix("- ")?;
    Some(rest)
}

/// ラベル付きリストの行か(`項目:: 値`)。
/// マクロ(`名前:対象[…]`)と紛れないよう `:: ` を見ます
fn is_labelled(l: &str) -> bool {
    desc_marker(l).is_some()
}

/// ラベル付きリストの行なら、その印(`::` `:::` `::::` `;;`)と項目の終わりの位置。
/// 印の後ろは空白か行末(`項目::` だけの行は、説明が次の行に来る形)。
/// 項目には空白が入ってよい(`Backend names:: …`。本家の作法。2026-09-02)
fn desc_marker(l: &str) -> Option<(&'static str, usize)> {
    let ts = l.trim_start();
    for m in ["::::", ":::", "::", ";;"] {
        let mut from = 0;
        while let Some(k) = ts[from..].find(m) {
            let i = from + k;
            let after = &ts[i + m.len()..];
            let ok_after = after.is_empty() || after.starts_with(' ');
            let term = &ts[..i];
            if ok_after && i > 0 && !term.contains('[') && !term.ends_with(':') && !term.ends_with(';') {
                return Some((m, i));
            }
            from = i + 1;
        }
    }
    None
}

/// 項目だけで説明の無いラベル付きリストの行か(`項目::`)。説明は次の行に続く
fn desc_term_only(l: &str) -> bool {
    match desc_marker(l) {
        Some((m, i)) => l.trim_start()[i + m.len()..].trim().is_empty(),
        None => false,
    }
}

/// その行が註記なら (スタイル名, 中身) を返す
fn is_admon(l: &str) -> Option<(&'static str, &str)> {
    let ts = l.trim_start();
    for (i, mark) in ADMONITION.iter().enumerate() {
        if let Some(rest) = ts.strip_prefix(mark) {
            if let Some(body) = rest.strip_prefix(' ') {
                return Some((ADMON_STYLE[i], body));
            }
        }
    }
    None
}

/// スタイル名から註記の印を引く(書くとき)
fn admon_mark(name: &str) -> Option<&'static str> {
    ADMON_STYLE.iter().position(|n| *n == name).map(|i| ADMONITION[i])
}

/// 塊の区切り(asciidoctor の `DELIMITED_BLOCKS`)。**うちが意味を知っている
/// `____`(引用)と `|===`(表)は入れません。**
///
/// 表は `docs/sekkei/asciidoctor-syntax.json` に写してあり、
/// `tools/adoc_syntax_check.py` が本家の表とずれていないか見ます
/// (2026-08-18 発注者「表示については、こちらからとりこんだら」)。
const DELIMITED: &[(&str, &str)] = &[
    ("----", "コードの塊(----)"),
    ("....", "字のまま出す塊(....)"),
    ("====", "例の塊(====)"),
    ("****", "傍注の塊(****)"),
    ("++++", "そのまま通す塊(++++)"),
    ("////", "覚え書きの塊(////)"),
    ("~~~~", "開いた塊(~~~~)"),
    ("--", "開いた塊(--)"),
    ("```", "コードの塊(```)"),
    (",===", "表(,===)"),
    (":===", "表(:===)"),
    ("!===", "表(!===)"),
];

/// その行が区切りか。**印は伸ばせます**(`-----` も `----` と同じ)。
/// 4字の印は同じ字の並び、それ以外はちょうどその字。
fn is_delim(t: &str, mark: &str) -> bool {
    let head = mark.chars().next().unwrap_or(' ');
    if mark.chars().count() == 4 && mark.chars().all(|c| c == head) {
        return t.chars().count() >= 4 && t.chars().all(|x| x == head);
    }
    t == mark
}


/// **1つのファイルに入っている文書を全部読む**(2026-08-19 発注者)。
///
/// 同時に送る請求書の原稿をまとめて置く、といった使い方のためです。
/// *1枚ずつが独立した文書*で、部や章のような「1つの文書の一部」ではありません。
///
/// 切れ目は `= 題` です。新しい印は足していません — いまも1枚の文書は
/// `= 題` から始まるので、*それが何度も出てきたらその数だけ文書がある*、
/// というだけの決めです。
///
/// 最初の `= 題` より前にある行(`:doctype: book` などの属性)は、
/// 1枚目の文書の物になります。`:doctype: book` は[`write_many`]が付ける
/// 印なので、読むときに落とします(二重に付かないようにするため)。
pub fn parse_many(src: &str) -> Result<Vec<Document>, String> {
    let block = split_docs(src);
    let mut out = Vec::with_capacity(block.len());
    for s in block {
        out.push(parse(&s)?);
    }
    Ok(out)
}

/// [`parse_many`] の帳簿つき。読めなかった書き方を数えて返します。
pub fn parse_many_full(src: &str) -> Result<(Vec<Document>, Vec<String>), String> {
    let mut docs = Vec::new();
    let mut ledger = Vec::new();
    for s in split_docs(src) {
        let (d, r) = parse_full(&s)?;
        docs.push(d);
        for x in r {
            if !ledger.contains(&x) {
                ledger.push(x);
            }
        }
    }
    Ok((docs, ledger))
}

/// 何枚もの文書を1つのファイルの字にする。
///
/// 2枚以上あるときは頭に `:doctype: book` を付けます。本家はこれが無いと
/// 2枚目の `= 題` を誤りとして扱います(2026-08-19 に確かめました)。
///
/// **名前の無い文書には番号で名前を付けます**(`文書 2`)。名前が無いと
/// 読み直したときに切れ目が分からず、画面のタブにも出せないためです。
pub fn write_many(docs: &[Document]) -> String {
    if docs.len() <= 1 {
        return docs.first().map(write).unwrap_or_default();
    }
    let mut out = String::new();
    for (i, d) in docs.iter().enumerate() {
        let mut s = write(d);
        if !has_heading(d) {
            s = format!("= 文書 {}\n{}", i + 1, s);
        }
        // **`[discrete]` を付けます。** これが無いと本家が「部には節が要る」と
        // 警告します(2026-08-19 発注者「警告が出ないように考えろ」)
        out.push_str(DOC_SEP_MARK);
        out.push('\n');
        out.push_str(s.trim_end());
        out.push_str("\n\n");
    }
    out
}

/// 文書の切れ目に付ける印。本家の「節ではない見出し」の書き方です。
const DOC_SEP_MARK: &str = "[discrete]";

fn has_heading(d: &Document) -> bool {
    matches!(d.blocks.first(), Some(Block::Para(p)) if p.style == ParaStyle::Title)
        || !d.props.title.is_empty()
}

/// その行が文書の切れ目(`= 題`)か。行頭にあることが要ります。
fn is_doc_title(l: &str) -> bool {
    l.starts_with("= ") && l.len() > 2
}

/// 字を文書ごとに切る。
///
/// **塊の中の `= ` では切りません。** 例の塊(`====`)や字のまま出す塊
/// (`....`)、表(`|===`)の中に `= 題` と書いてあっても、それは中身です。
fn split_docs(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    // いま開いている塊の印(None なら塊の外)
    let mut opened: Option<String> = None;
    let mut saw_title = false;
    // 直前の行が `[discrete]` の切れ目だったか(次の題行で二重に切らないため)
    let mut split_by_mark = false;
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_end();
        let content = t.trim();
        match &opened {
            Some(mark) => {
                if is_delim(content, mark) {
                    opened = None;
                }
            }
            None => {
                if content == "|===" || content == "____" {
                    opened = Some(content.to_string());
                } else if let Some((mark, _)) = DELIMITED.iter().find(|(d, _)| is_delim(content, d)) {
                    opened = Some((*mark).to_string());
                } else if content == DOC_SEP_MARK && next_is_title(&lines, i) {
                    // **切れ目の印。** 印そのものは持ち越しません。
                    //
                    // **前置きだけの塊は文書にしません**(2026-08-24)。
                    // `:doctype: book` を頭に置いた形([`write_many`] が書く形)
                    // では、1つ目の `[discrete]` より前が属性だけになります。
                    // そこで切ると、*中身の無い節が1つ増えます* — 節タブが
                    // 1つ余分に出る形で、実際に踏みました
                    if saw_title && !cur.trim().is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    if !saw_title {
                        // **前置き(属性だけの塊)は捨てます**(2026-08-24)。
                        // `:doctype: book` は書き手が付ける物なので、読み手が
                        // 抱えると2つの困り事になります — 中身の無い節が1つ
                        // 増えるか、1枚目の `= 題` が1行目に来なくなって
                        // *題が本文の字に落ちます*(実際に踏みました)
                        cur.clear();
                    }
                    saw_title = true;
                    // **次の行の題で、もう一度切らせません。** 印で切った直後に
                    // 題の枝がまた切ると、*前置きだけの塊が1つ増えます*
                    split_by_mark = true;
                    i += 1;
                    continue;
                } else if is_doc_title(t) {
                    // 印の無い `= 題` でも切ります(手で書いたファイル)。
                    // **2枚目からが切れ目** — 1枚目の題より前は属性です
                    if saw_title && !split_by_mark {
                        out.push(std::mem::take(&mut cur));
                    }
                    saw_title = true;
                    split_by_mark = false;
                }
            }
        }
        cur.push_str(line);
        cur.push('\n');
        i += 1;
    }
    out.push(cur);
    // 中身の無い塊(末尾の空行だけ)は文書として数えません
    out.retain(|s| !s.trim().is_empty());
    // **頭の空行を落とします。** `= 題` が1行目に来ないと、読み手が
    // 文書の題として取らず、字のまま本文に落ちます
    for s in out.iter_mut() {
        while s.starts_with('\n') {
            s.remove(0);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// 覚え書きの行の次(覚え書きは飛ばす)が `= 題` か `:名前: 値` か
fn next_is_head(lines: &std::iter::Peekable<std::iter::Enumerate<std::str::Lines<'_>>>) -> bool {
    lines
        .clone()
        .map(|(_, l)| l.trim_end())
        .skip(1)
        .find(|l| !l.starts_with("//"))
        .is_some_and(|l| l.starts_with("= ") || (l.starts_with(':') && l[1..].contains(':')))
}

/// `[discrete]` の次(空行は飛ばす)が `= 題` か。
fn next_is_title(line: &[&str], at: usize) -> bool {
    line[at + 1..]
        .iter()
        .find(|l| !l.trim().is_empty())
        .is_some_and(|l| is_doc_title(l.trim_end()))
}

/// AsciiDoc(部分集合)→ 模型。**意味だけが入る** — 見た目の欄は触らない。
/// 形が壊れていれば Err、うちが扱わない書き方は [`parse_full`] の帳簿に出る
pub fn parse(src: &str) -> Result<Document, String> {
    parse_full(src).map(|(d, _)| d)
}

/// AsciiDoc(部分集合)→ (模型, 帳簿)。
///
/// **帳簿は「読めたけれど、うちの書き方ではないもの」の一覧です。**
/// 本家の AsciiDoc には、うちが扱わない書き方がたくさんあります(註記・
/// コードの塊・取り込みなど)。それらは字としては残りますが、**意味は
/// 落ちています**。黙って本文に化けさせると、書いた人は出来上がりを見るまで
/// 気づけません(2026-08-18。それまで8つ試して8つとも黙って化けていました)。
pub fn parse_full(src: &str) -> Result<(Document, Vec<String>), String> {
    let mut ledger: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut doc = Document::default();
    // 二行の見出し(下線)は先に `=` の形に直しておく(本家の古い書き方)
    let src_atx = setext_to_atx(src);
    let src: &str = &src_atx;
    let mut lines = src.lines().enumerate().peekable();
    let mut pending_bookmarks: Vec<String> = Vec::new();
    // 継いでいる段落(塊の番号と、貯めた原文)。段落の終わりでまとめて読む
    let mut pending: Option<(usize, String)> = None;
    let mut pending_break = false;
    let mut pending_style: Option<String> = None;
    let mut in_quote = false;
    let mut fresh_note = 0usize;
    // 直前の行が「継げる本文」だったか(空行と特別な行で倒れる)
    let mut prev_is_body = false;
    // **直前の行もラベル付きリストだったか。** 空行で切れた2つ目の一覧に
    // 印を付けるために要ります(印が無いと書き戻しで空行が消えて、
    // 別々の一覧が1つに繋がります)
    let mut prev_is_desc_list = false;
    // 字下げの段落(literal)の途中か
    let mut in_literal = false;
    // 直前に続いた空行の数
    let mut blank_run = 0usize;
    // 節。`[page-…]` の行で前の段落に節の印を付け、そこからの用紙を持つ
    let mut cur_page: Option<crate::PageSetup> = None;
    // 直前に節の印を付けた塊(次の行が `<<<` なら改ページする節にする)
    let mut sect_just: Option<usize> = None;

    // 文書の頭: `= 題名` と `:鍵: 値`。**空行までが頭**(本家の作法)
    let mut head_done = false;
    // 頭に入ったか(`= 題` か `:鍵:` を1つでも見たか)。見ていない文書の
    // 1行目を頭の行と誤らないための旗
    let mut in_head = false;
    while let Some((_, line)) = lines.peek().copied() {
        let l = line.trim_end();
        // 頭の前や中の覚え書き(`// tag::title[]` など)は、頭の原文の行として
        // 持ち越します。前は頭を打ち切っていたので、続く `= 題` が本文の
        // 見出しになり、その次の `:名前: 値` が題に混ざりました(2026-09-02)
        if !head_done && l.starts_with("//") && (in_head || next_is_head(&lines)) {
            doc.attrs.push((String::new(), l.to_string()));
            lines.next();
            continue;
        }
        if !head_done && doc.props.title.is_empty() && l.starts_with("= ") {
            let title = l[2..].trim().to_string();
            // **表題は本文の段落にもする**(2026-08-18)。文書の情報にしか
            // 入れないと紙面に出ず、開いた人には題名が消えて見えます。
            // `props.title` にも同じ字を入れて、docx の文書の情報と往復します
            doc.props.title = title.clone();
            let mut p = Paragraph {
                style: ParaStyle::Title,
                line_spacing: 1.0,
                ..Default::default()
            };
            p.runs.push(Run { text: title, size_pt: None, font: None, fmt: CharFormat::default() });
            doc.blocks.push(Block::Para(p));
            in_head = true;
            lines.next();
            continue;
        }
        if !head_done {
            if let Some(rest) = l.strip_prefix(':') {
                if let Some((k, v)) = rest.split_once(':') {
                    let (k, v) = (k.trim().to_string(), v.trim().to_string());
                    // **知らない名前も捨てません。** AsciiDoc の文書は頭に
                    // 属性を並べます(`:author:` `:revdate:` など)。捨てると、
                    // 普通の AsciiDoc を開いて保存しただけで消えます
                    // (2026-08-18。それまでは知らない名前で読むのをやめていました)
                    if k == "template" || k == "テンプレート" {
                        doc.template = Some(v.clone());
                    }
                    doc.attrs.push((k, v));
                    in_head = true;
                    lines.next();
                    continue;
                }
            }
            if l.is_empty() {
                head_done = true;
                lines.next();
                continue;
            }
            // **頭は空行まで続きます**(本家の作法)。`:鍵: 値` でない行
            // (著者の行など)も**そのまま持ち越します** — 前はここで頭を
            // 打ち切っていたので、後ろに並ぶ属性が本文に落ち、書き戻しで
            // 消えていました(2026-08-18、本家の README で見つけた)。
            // 鍵が空の項目は「原文のままの行」の印です
            if in_head {
                doc.attrs.push((String::new(), l.to_string()));
                lines.next();
                continue;
            }
            // 頭の印が1つも無い文書は、そのまま本文へ
        }
        break;
    }

    while let Some((ln, line)) = lines.next() {
        let l = line.trim_end();
        if l.is_empty() {
            prev_is_body = false; // 空行が段落の切れ目
            prev_is_desc_list = false;
            blank_run += 1;
            continue;
        }
        // この行の前にあった空行の数(原文のままの行が、書き戻しで空きを保つのに使う)
        let blank_before = std::mem::take(&mut blank_run);
        // 印(箇条書き・番号・項目)は字下げしてあってもよい(本家の作法)。
        // 字下げの段落(literal)と取り違えないよう、先に見ておく
        let lt = l.trim_start();
        let list_like = is_bullet(lt, '*').is_some()
            || is_dash_bullet(lt).is_some()
            || is_bullet(lt, '.').is_some()
            || is_numbered_marker(lt).is_some()
            || is_labelled(lt)
            || vendor_only_syntax(lt).is_some_and(|(_, r)| r == "コールアウト");
        // **字下げの段落(literal)。** 段落の頭の行が空白で始まれば、その段落は
        // 字のまま組む物で、行も継ぎません(本家の作法)。原文のまま持ちます
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if indented && !list_like && (!prev_is_body || in_literal) {
            *ledger.entry("字下げの段落(literal)").or_default() += 1;
            let mut p = Paragraph {
                line_spacing: 1.0,
                style_id: Some("字下げ".to_string()),
                raw_adoc: Some(if next_is_blank(&mut lines) { format!("{l}\n") } else { l.to_string() }),
                ..Default::default()
            };
            p.runs.push(Run { text: l.to_string(), size_pt: None, font: None, fmt: CharFormat::default() });
            doc.blocks.push(Block::Para(p));
            prev_is_body = false;
            prev_is_desc_list = false;
            in_literal = true;
            continue;
        }
        in_literal = false;
        // **節の用紙**(asciidoctor-pdf の書き方)。この行で前の段落までが
        // 1つの節になり、ここから先は新しい用紙です。次の行が `<<<` なら
        // 改ページし、無ければ続き(continuous)の節です
        if let Some(attrs) = page_attrs(l) {
            let base = match cur_page {
                Some(pg) => pg,
                None => match doc.page {
                    Some(pg) => pg,
                    None => page_from_attrs(&doc.attrs).map_err(|e| format!("頭の属性: {e}"))?.unwrap_or_default(),
                },
            };
            let brk = crate::SectionBreak { raw: String::new(), page: base, continuous: true };
            match doc.blocks.last_mut() {
                Some(Block::Para(p)) if p.sect.is_none() => p.sect = Some(brk),
                _ => doc.blocks.push(Block::Para(Paragraph { line_spacing: 1.0, sect: Some(brk), ..Default::default() })),
            }
            sect_just = Some(doc.blocks.len() - 1);
            cur_page = Some(apply_page_attrs(base, &attrs).map_err(|e| format!("{} 行目: {e}", ln + 1))?);
            prev_is_body = false;
            prev_is_desc_list = false;
            continue;
        }
        if let Some((what, role)) = vendor_only_syntax(l) {
            // **種類が分かる塊は帳簿に載せません**(2026-09-03)。コード・字のまま・
            // 例・傍注・覚え書き・そのまま通す塊は中身に種類の名前が付き、画面と
            // 紙と Web で種類どおりに出ます。開いた塊(`--` `~~~~`)と表の別記法は
            // まだ中身を読まないので載せたままです
            let wakaru = role == "塊の区切り" && !what.starts_with("開いた塊") && !what.starts_with("表(");
            if !wakaru {
                *ledger.entry(what).or_default() += 1;
            }
            // **原文のまま持ち越し、役割の名前を付けます。**
            // 意味は分からなくても、字は壊さず返し、テンプレートで見た目を
            // 決められるようにします(2026-08-18)。役割が空の物
            // (属性の参照)は、行そのものはうちの書き方なので普通に読みます
            if !role.is_empty() {
                // 後ろに空行があればそれも原文に含める(塊の中で行が
                // 離れず、塊の外では離れる — 元のままに返るのはこの形だけ)
                // **一覧の中に来る行は、前の空行の数と錨も原文に含めます。**
                // 空行の数は本家にとって意味(どの項目に付くか・一覧が切れるか)
                // なので、書き戻しで数を保ちます(2026-09-03)
                let keeps_blanks = matches!(role, "覚え書き" | "コールアウト" | "指定の行");
                // 直前の原文のままの行が後ろの空行を1つ抱えていれば、その分は引く
                let prev_took_blank = matches!(doc.blocks.last(), Some(Block::Para(q))
                    if q.raw_adoc.as_deref().is_some_and(|r| r.ends_with('\n')));
                let lead: String = if keeps_blanks {
                    let mut t = "\n".repeat(blank_before.saturating_sub(prev_took_blank as usize));
                    for bm in pending_bookmarks.drain(..) {
                        t.push_str(&format!("[[{bm}]]\n"));
                    }
                    t
                } else {
                    String::new()
                };
                let raw = |l: &str, blank_line: bool, doc: &mut Document| {
                    let mut p = Paragraph {
                        line_spacing: 1.0,
                        style_id: Some(role.to_string()),
                        raw_adoc: Some(format!("{lead}{}", if blank_line { format!("{l}\n") } else { l.to_string() })),
                        ..Default::default()
                    };
                    // 塊の題は先頭の `.` を取った字で見せます(原文は raw_adoc)
                    let shown = if role == "塊の題" { l.trim_start().trim_start_matches('.').to_string() } else { l.to_string() };
                    p.runs.push(Run {
                        text: shown,
                        size_pt: None,
                        font: None,
                        fmt: CharFormat::default(),
                    });
                    doc.blocks.push(Block::Para(p));
                };
                let empty = |it: &mut std::iter::Peekable<_>| -> bool { next_is_blank(it) };
                raw(l, empty(&mut lines), &mut doc);
                // 区切りの塊(`----` など)は**閉じるまでまるごと**持ち越す
                prev_is_body = false; // 原文のままの行に続きを繋がない
                prev_is_desc_list = false;
                if role == "塊の区切り" {
                    let mark = l.trim_end().to_string();
                    // 中身のスタイル名。直前の指定の行(`[source,python]` など)は
                    // 今しがた raw() が押した区切りの1つ前に居ます
                    let spec: Option<String> = match doc.blocks.len().checked_sub(2).map(|i| &doc.blocks[i]) {
                        Some(Block::Para(q)) if q.style_id.as_deref() == Some("指定の行") => {
                            Some(q.runs.iter().map(|r| r.text.as_str()).collect())
                        }
                        _ => None,
                    };
                    let inner_style = block_inner_style(&mark, spec.as_deref());
                    while let Some((_, l2)) = lines.next() {
                        let closing = l2.trim_end() == mark;
                        if closing {
                            // 閉じの印。後ろの空行も原文に含めて持ち越す
                            let blank_line = empty(&mut lines);
                            let mut q = Paragraph {
                                line_spacing: 1.0,
                                style_id: Some("塊の区切り".to_string()),
                                raw_adoc: Some(if blank_line {
                                    format!("{}\n", l2.trim_end())
                                } else {
                                    l2.trim_end().to_string()
                                }),
                                ..Default::default()
                            };
                            q.runs.push(Run {
                                text: l2.trim_end().to_string(),
                                size_pt: None,
                                font: None,
                                fmt: CharFormat::default(),
                            });
                            doc.blocks.push(Block::Para(q));
                            break;
                        }
                        // **塊の中身は画面で直せます**(2026-08-18)。
                        // 原文のまま持ち越すのをやめ、普通の段落にしました。
                        // 字は読んだまま入れます — 塊の中の `*` は太字の印では
                        // ないので、行の中の書き方として読んではいけません。
                        // 空行も1つの段落として残します(前は落ちていました)
                        doc.blocks.push(Block::Para(Paragraph {
                            line_spacing: 1.0,
                            style_id: Some(inner_style.to_string()),
                            runs: vec![Run {
                                text: l2.trim_end().to_string(),
                                size_pt: None,
                                font: None,
                                fmt: CharFormat::default(),
                            }],
                            ..Default::default()
                        }));
                    }
                }
                continue;
            }
        }
        if l == "____" {
            in_quote = !in_quote;
            continue;
        }
        // **一覧の続き `+`。** 次の段落を上の項目に付ける印。原文のまま持ち、
        // 前後の段落とは継がない(2026-09-03。前は次の段落の字に混ざっていた)
        if l == "+" {
            flush_pending(&mut pending, &mut doc, &mut fresh_note)?;
            // 前の空行の数を保つ(空行の数で、どの段の項目に付くかが決まる)
            let prev_took_blank = matches!(doc.blocks.last(), Some(Block::Para(q))
                if q.raw_adoc.as_deref().is_some_and(|r| r.ends_with('\n')));
            let mut p = Paragraph {
                line_spacing: 1.0,
                style_id: Some("一覧の続き".to_string()),
                raw_adoc: Some(format!("{}+", "\n".repeat(blank_before.saturating_sub(prev_took_blank as usize)))),
                ..Default::default()
            };
            p.runs.push(Run { text: "+".to_string(), size_pt: None, font: None, fmt: CharFormat::default() });
            doc.blocks.push(Block::Para(p));
            prev_is_body = false;
            continue;
        }
        if l == "<<<" {
            // `[page-…]` の直後の `<<<` は、その節の始め方(改ページ)です
            if sect_just.is_some() && sect_just == doc.blocks.len().checked_sub(1) {
                if let Some(Block::Para(p)) = doc.blocks.last_mut() {
                    if let Some(sb) = p.sect.as_mut() {
                        sb.continuous = false;
                    }
                }
                sect_just = None;
            } else {
                pending_break = true;
            }
            continue;
        }
        if let Some(name) = l.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            pending_bookmarks.push(name.to_string());
            continue;
        }
        // 段落のスタイル名(次の塊に掛かる)
        if let Some(name) = l.strip_prefix("[.").and_then(|s| s.strip_suffix(']')) {
            if !name.is_empty() && !name.contains(['[', ']', '#', ' ']) {
                pending_style = Some(name.to_string());
                continue;
            }
        }
        if l == "|===" {
            let mut rows: Vec<&str> = Vec::new();
            let mut closed = false;
            // **1行目の後ろの空行が「見出しの行」の印**(AsciiDoc の作法)
            let mut heading_line = false;
            // 空行を見た時点で本当の行が何行あったか(印の行は数えません)
            let mut real_line = 0usize;
            // いま `a|` のセルの中にいるか(続きの行では持ち越します)
            let mut in_anchor = false;
            for (_, tl) in lines.by_ref() {
                // **末尾は半角の空きだけ落とします。** 全角の空白(U+3000)は
                // 日本語の様式では字下げなので、落とすと見た目が変わります
                let tl = tl.trim_end_matches([' ', '\t', '\r']);
                if tl == "|===" {
                    closed = true;
                    break;
                }
                if tl.is_empty() {
                    // 1行目の後ろの空行は**見出しの印**(今までどおり)。
                    // ただし `a|` のセルの中の空行は段落の切れ目なので、
                    // 見出しの印と取り違えません
                    if real_line == 1 && !in_anchor {
                        heading_line = true;
                        continue;
                    }
                    // それ以外の空行は `a|` のセルの中の段落の切れ目かも
                    // しれないので、印として残して後で見分けます
                    if real_line > 0 {
                        rows.push(TABLE_BLANK_ROW);
                    }
                    continue;
                }
                real_line += 1;
                // セルの頭がある行だけ、種類を取り直します
                if let Some(a) = cell_kind(tl) {
                    in_anchor = a;
                }
                rows.push(tl);
            }
            if !closed {
                return Err(format!("{} 行目: |=== が閉じていません", ln + 1));
            }
            // **桁の数は先に `[cols=]` から取ります。** `a|` のセルは中身が
            // 次の行に続くので、「最初の行のセルの数」では数えられません
            let col_spec = match doc.blocks.last() {
                Some(Block::Para(prev)) if prev.style_id.as_deref() == Some("指定の行") => {
                    cols_of(prev.raw_adoc.as_deref().unwrap_or("")).map(|ratio| ratio.len())
                }
                _ => None,
            };
            let mut t = parse_table_lines(&rows, &mut doc, &mut fresh_note, col_spec)?;
            t.header_row = heading_line;
            // **直前の `[cols="1,3"]` は表の物です**(2026-08-18)。原文のまま
            // 持ち越した段落として残っているので、取り込んで消します。
            // 残したままだと、書くときに二重に出ます
            if let Some(Block::Para(prev)) = doc.blocks.last() {
                if prev.style_id.as_deref() == Some("指定の行") {
                    if let Some(ratio) = cols_of(prev.raw_adoc.as_deref().unwrap_or("")) {
                        t.col_ratio = ratio;
                        doc.blocks.pop();
                        // **読めたので帳簿から下げます**(2026-08-19、表の題と
                        // 同じ作法)。桁の割合は表に取り込んだので、
                        // 「読み飛ばした」と言うと嘘になります
                        if let Some(n) = ledger.get_mut("塊の指定([…])") {
                            *n -= 1;
                            if *n == 0 {
                                ledger.remove("塊の指定([…])");
                            }
                        }
                    }
                }
            }
            // **直前の `.題` も表の物です**(2026-08-18)。表の名前になるので、
            // 原文のまま持ち越すのをやめて表に入れます
            if let Some(Block::Para(prev)) = doc.blocks.last() {
                if prev.style_id.as_deref() == Some("塊の題") {
                    // 字は `.` を取った題そのもの(2026-09-03。前は原文の字だった)
                    let title: String = prev.runs.iter().map(|r| r.text.as_str()).collect();
                    if !title.trim().is_empty() {
                        let name = title.trim();
                        t.title = Some(name.to_string());
                        doc.blocks.pop();
                        // **読めたので帳簿から下げます。** 表の題は取り込んだ
                        // ので、「読めなかった」と言うと嘘になります
                        if let Some(n) = ledger.get_mut("塊の題(.題)") {
                            *n -= 1;
                            if *n == 0 {
                                ledger.remove("塊の題(.題)");
                            }
                        }
                    }
                }
            }
            // **直前の `[.name]` は表の役割です**(2026-08-26)
            if let Some(role) = pending_style.take() {
                t.role = Some(role);
            }
            doc.blocks.push(Block::Table(t));
            continue;
        }
        if let Some(rest) = l.strip_prefix("image::") {
            let (path, _attrs) = split_macro_target(rest)
                .ok_or_else(|| format!("{} 行目: image:: の形が読めません", ln + 1))?;
            let mut p = base_para(&mut pending_bookmarks, &mut pending_break, &mut pending_style);
            p.images_new.push(InlineImage {
                bytes: std::sync::Arc::new(Vec::new()),
                w_mm: 0.0,
                h_mm: 0.0,
                tex: None,
                src: Some(path.to_string()),
                off: 0,
            });
            doc.blocks.push(Block::Para(p));
            continue;
        }
        if let Some(rest) = l.strip_prefix("stem:[") {
            let tex = rest
                .strip_suffix(']')
                .ok_or_else(|| format!("{} 行目: stem:[ が閉じていません", ln + 1))?;
            let mut p = base_para(&mut pending_bookmarks, &mut pending_break, &mut pending_style);
            p.images_new.push(InlineImage {
                bytes: std::sync::Arc::new(Vec::new()),
                w_mm: 0.0,
                h_mm: 0.0,
                tex: Some(tex.to_string()),
                src: None,
                off: 0,
            });
            doc.blocks.push(Block::Para(p));
            continue;
        }
        // 見出し・リスト・本文
        let mut p = base_para(&mut pending_bookmarks, &mut pending_break, &mut pending_style);
        let body = if let Some(rest) = heading_of(l) {
            let (n, text) = rest;
            p.style = ParaStyle::Heading(n);
            text
        } else if let Some((name, text)) = is_admon(l) {
            // 註記(`NOTE: 文`)。**印は字に残しません** — どれなのかは
            // 段落のスタイルが持ちます(2026-08-18)
            p.style_id = Some(name.to_string());
            text
        } else if is_labelled(l) {
            // ラベル付きリスト(`項目:: 値`)。**`::` は字のまま残します** —
            // 画面で項目も値も直せて、書き戻しもそのままです(2026-08-18)
            // **空行で切れた2つ目の一覧には、始めの印を付けます。**
            // 付けないと、書き戻しで空行が落ちて1つに繋がります
            let cont = prev_is_desc_list
                || !matches!(doc.blocks.last(), Some(Block::Para(q))
                             if q.style_id.as_deref().is_some_and(is_desc_list));
            p.style_id = Some(if cont {
                "説明のリスト".to_string()
            } else {
                DESC_LIST_START.to_string()
            });
            l
        } else if let Some((tab, rest)) = is_bullet(lt, '*') {
            // **入れ子の箇条書き**(`**` `***`)。AsciiDoc は印の数が段です
            p.list = ListKind::Bullet;
            p.indent = tab;
            rest
        } else if let Some(rest) = is_dash_bullet(lt) {
            // **`-` の箇条書き。** AsciiDoc は `*` と `-` の両方を受けます。
            // `-` は1段目だけで、入れ子にはできません(本家の決め)。
            // 見ていなかったので、2つの項目が1つの段落に潰れ、印の `-` も
            // 字のまま出ていました(2026-08-27 に実物の PDF で気づいた)。
            // 印は覚えておいて、書くときも `-` で返します(`*` と混ぜると
            // 本家は入れ子と読むので、変えてはいけない。2026-09-03)
            p.list = ListKind::Bullet;
            p.indent = 0;
            p.list_text = Some("-".to_string());
            rest
        } else if let Some((tab, rest)) = is_bullet(lt, '.') {
            p.list = ListKind::Number;
            p.indent = tab;
            rest
        } else if let Some((mark, rest)) = is_numbered_marker(lt) {
            // 番号を明示した一覧(`7.` `a.` `i)` `IV)`)。印を覚えて、そのまま返す
            p.list = ListKind::Number;
            p.indent = 0;
            p.list_text = Some(mark.to_string());
            rest
        } else {
            l
        };
        if in_quote {
            p.style = ParaStyle::Quote;
        }
        // **継ぐかどうかは、字を読む前に決まります**(見出しか・箇条書きか・
        // 名前つきか)。継ぐなら字を貯めて、段落の終わりでまとめて読みます
        let continued = prev_is_body
            && p.style == ParaStyle::Body
            && p.list == ListKind::None
            && p.bookmarks.is_empty()
            && p.style_id.is_none()
            && !p.page_break_before;
        if !continued {
            flush_pending(&mut pending, &mut doc, &mut fresh_note)?;
        }
        // 継ぐ行の頭の空白は落とします(続きの印であって、字ではありません)
        let body = if continued { body.trim_start() } else { body };
        // **続く行は同じ段落に継ぐ**(AsciiDoc の作法)。段落の切れ目は空行で、
        // 行の折り返しではありません。80 桁で折った普通の AsciiDoc を開くと、
        // 前は行ごとにバラバラの段落になり、保存で空行が入って構造が変わって
        // いました(2026-08-18)。**継ぎ目に空白は入れません** — 日本語の文を
        // 行で折っても語が割れないようにするためです
        // **1行で1つと決まっている段落には、次の行を継ぎません。**
        // 註記(`NOTE: 文`)とラベル付きリスト(`項目:: 値`)がそれです。
        //
        // 利用者が付けた名前(`[.強調の囲み]`)は別です。**普通の段落と同じで、
        // 何行にもわたって書けます**(2026-08-18 に見本を揃えたとき、
        // 2行目が別の段落になって気づきました)
        // **箇条書きの項目にも、続きの行は継ぎます。** AsciiDoc では
        // 空行までが1つの項目です(2026-08-18 に設計文書を読み返して
        // 見つけました — 続きの行が別の段落になり、字下げの塊に化けて
        // いました)
        // **見出しには続きの行を継ぎません**(見出しは1行の物)。継ぐと
        // 直後の `:名前: 値` や本文が題名に混ざります(2026-09-02 に本家の
        // 手引きで見つけた)
        // **項目だけのラベル付きリスト(`項目::`)には、次の行(説明)を継ぎます**
        // (2026-09-03。本家は説明を次の行に書く形も受ける)
        prev_is_body = p.raw_adoc.is_none()
            && (!p.style_id.as_deref().is_some_and(one_per_line)
                || (p.style_id.as_deref().is_some_and(is_desc_list) && desc_term_only(l)))
            && !matches!(p.style, ParaStyle::Heading(_) | ParaStyle::Title);
        prev_is_desc_list = p.style_id.as_deref().is_some_and(is_desc_list);
        if continued {
            if let Some((_, src)) = pending.as_mut() {
                // **行末の ` +` は AsciiDoc の改行**です。段落の中の改行として
                // 字に持ち、書くときに ` +` に戻します(2026-09-02)
                if let Some(head) = src.strip_suffix(" +") {
                    src.truncate(head.len());
                    src.push('\n');
                } else {
                    src.push_str(seam_src(src, body));
                }
                src.push_str(body);
                continue;
            }
        }
        doc.blocks.push(Block::Para(p));
        pending = Some((doc.blocks.len() - 1, body.to_string()));
    }
    flush_pending(&mut pending, &mut doc, &mut fresh_note)?;
    // 最後の節の用紙。節が無ければ頭の `:page-…:` だけを見ます(無ければ None のまま =
    // テンプレートが決めます)
    match cur_page {
        Some(pg) => doc.page = Some(pg),
        None => {
            if doc.page.is_none() {
                doc.page = page_from_attrs(&doc.attrs).map_err(|e| format!("頭の属性: {e}"))?;
            }
        }
    }
    let ledger = ledger
        .into_iter()
        .map(|(k, n)| if n > 1 { format!("{k} × {n}") } else { k.to_string() })
        .collect();
    Ok((doc, ledger))
}

fn base_para(
    bookmarks: &mut Vec<String>,
    brk: &mut bool,
    style: &mut Option<String>,
) -> Paragraph {
    Paragraph {
        bookmarks: std::mem::take(bookmarks),
        page_break_before: std::mem::take(brk),
        style_id: style.take(),
        ..Default::default()
    }
}

/// `== 見出し` → (1, "見出し")。`=` の数 − 1 が水準(1〜3)
/// 見出しの行か。返るのは(段, 字)。
///
/// **段は5まで**(2026-08-18)。AsciiDoc の `=` は表題で、`==` から見出しが
/// 始まるので、`======` が見出し5です。本家はここまでしかありません
/// **二行の見出し(下線)を `=` の形に直します**(本家の古い書き方。2026-09-03)。
///
/// 字の行の次に同じ字(`=` `-` `~` `^` `+`)だけの行があり、長さが字の行と
/// 1字以内で揃っていれば見出しです。`=` が表題、`-` が `==`、`~` が `===`、
/// `^` が `====`、`+` が `=====`。区切りの塊の中は見ません。
/// 書くときは `=` の形になるので、字は変わりますが本家の組み方は同じです
fn setext_to_atx(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::with_capacity(src.len());
    let mut open: Option<&str> = None;
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim_end();
        if let Some(mark) = open {
            if is_delim(l, mark) {
                open = None;
            }
            out.push_str(lines[i]);
            out.push('\n');
            i += 1;
            continue;
        }
        let underline = lines.get(i + 1).map(|n| n.trim_end()).and_then(|n| {
            let c = n.chars().next()?;
            if !"=-~^+".contains(c) || !n.chars().all(|x| x == c) {
                return None;
            }
            let (a, b) = (l.chars().count() as i64, n.chars().count() as i64);
            if b < 2 || (a - b).abs() > 1 {
                return None;
            }
            Some(c)
        });
        let candidate = !l.is_empty()
            && heading_of(l).is_none()
            && !l.starts_with(['[', ':', '<', '|', '.', '*', '-', '+', '/', '=', '~', '^', ' ', '\t'])
            && !is_labelled(l);
        if let (Some(c), true) = (underline, candidate) {
            let level = "=-~^+".find(c).unwrap_or(0);
            out.push_str(&"=".repeat(level + 1));
            out.push(' ');
            out.push_str(l);
            out.push('\n');
            i += 2;
            continue;
        }
        if l == "|===" || l == "____" {
            open = Some(if l == "|===" { "|===" } else { "____" });
        } else if let Some((mark, _)) = DELIMITED.iter().find(|(d, _)| is_delim(l, d)) {
            open = Some(mark);
        }
        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }
    if !src.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn heading_of(l: &str) -> Option<(u8, &str)> {
    for n in (1..=5u8).rev() {
        let mark = "=".repeat(n as usize + 1) + " ";
        if let Some(rest) = l.strip_prefix(&mark) {
            return Some((n, rest));
        }
    }
    None
}

/// `[cols="1,3"]` の行から桁の割合を読む。読めなければ `None`。
///
/// 本家には `cols="1,2,3"`(比)のほかに `cols="3*"`(同じ幅を3つ)や
/// `cols="<,^,>"`(揃え)もあります。**うちが読むのは比だけ**で、それ以外の
/// 指定が混じっていたら手を出しません(半分だけ効かせると、書いた人は
/// 何が効いたのか分からなくなります)
fn cols_of(line: &str) -> Option<Vec<f32>> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    let v = inner.trim().strip_prefix("cols=")?.trim();
    let v = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(v);
    // `3*` は「同じ幅を3つ」
    if let Some(n) = v.strip_suffix('*').and_then(|s| s.trim().parse::<usize>().ok()) {
        return (1..=8).contains(&n).then(|| vec![1.0; n]);
    }
    let mut out = Vec::new();
    for part in v.split(',') {
        let p = part.trim();
        if p.is_empty() {
            return None;
        }
        out.push(p.parse::<f32>().ok().filter(|x| *x > 0.0)?);
    }
    (!out.is_empty() && out.len() <= 16).then_some(out)
}

/// `path[attrs]` を (path, attrs) に割る
fn split_macro_target(s: &str) -> Option<(&str, &str)> {
    let open = s.find('[')?;
    let close = s.rfind(']')?;
    (close == s.len() - 1).then(|| (&s[..open], &s[open + 1..close]))
}

/// インラインの印を run の並びへ。中身は [`crate::inline`](本家と同じ順で
/// 置き換えます)
fn parse_inline(text: &str, doc: &mut Document, fresh_note: &mut usize) -> Result<Vec<Run>, String> {
    crate::inline::parse(text, doc, fresh_note)
}

/// 継いだ段落の字を、まとめて run の並びにする。
///
/// **行ごとに読まず、段落をまるごと読みます。** 本家は段落を1つの
/// 文字列にして印を探すので、一重の印の前後の条件(直前が字でない・
/// 直後が字でない)は行をまたいで効きます(2026-09-02)
fn flush_pending(
    pending: &mut Option<(usize, String)>,
    doc: &mut Document,
    fresh_note: &mut usize,
) -> Result<(), String> {
    if let Some((idx, src)) = pending.take() {
        let runs = parse_inline(&src, doc, fresh_note)?;
        if let Some(Block::Para(p)) = doc.blocks.get_mut(idx) {
            p.runs = runs;
        }
    }
    Ok(())
}

/// 行を継ぐときの継ぎ目を原文の字で決める。印の字は飛ばして、
/// 前の行の最後の字と次の行の最初の字を見ます
fn seam_src(before: &str, after: &str) -> &'static str {
    // 行末が裸の URL なら、次の行の字を URL に食わせないために空白を入れます
    // (URL が `]` や句読点で閉じていれば、そのまま継ぎます)
    let last_word = before.rsplit(char::is_whitespace).next().unwrap_or("");
    let url_open = |c: char| c.is_ascii_alphanumeric() || "/_-#%~.?=&+".contains(c);
    if last_word.contains("://") && last_word.chars().last().is_some_and(url_open) {
        return " ";
    }
    let is_mark = |c: &char| crate::inline::MARK_CHARS.contains(c);
    let a: String = before.chars().rev().find(|c| !is_mark(c)).into_iter().collect();
    let b: String = after.chars().find(|c| !is_mark(c)).into_iter().collect();
    seam(&a, &b)
}

fn parse_table_lines(
    rows_src: &[&str],
    doc: &mut Document,
    fresh_note: &mut usize,
    col_spec: Option<usize>,
) -> Result<Table, String> {
    let mut t = Table::default();
    // **セルの中身は次の行に続きます**(本家の作法。2026-08-18 に直した)。
    // `|` で始まらない行は前の行の続きです。前は「表の行はセルごとに | で
    // 始める」と断っていたので、本家の手引き 176 枚のうち 11 枚が開けません
    // でした
    let mut joined: Vec<String> = Vec::new();
    for l in rows_src {
        // **空行は `a|` のセルの中だけ段落の切れ目にします。**
        // ほかの空行は今までどおり捨てます(表の見た目のための空行なので)
        if *l == TABLE_BLANK_ROW {
            if let Some(before) = joined.last_mut() {
                if last_cell_is_adoc(before) && !before.ends_with("\n\n") {
                    before.push_str("\n\n");
                }
            }
            continue;
        }
        let head = l.trim_start();
        if joined.is_empty() || head.starts_with('|') || cell_has_attrs(head) {
            joined.push((*l).to_string());
        } else if let Some(before) = joined.last_mut() {
            // 続きの行。**全角の空白は字下げなので残します**
            let cont = trim_edges(l);
            // **行末の ` +` は AsciiDoc の改行**です(2026-08-31)。段落の
            // 切れ目ではなく1つの段落の中の改行なので、印を落として
            // 改行で継ぎます。表計算のセルの「区　分 / Type」がこの形です
            if before.ends_with(" +") {
                before.truncate(before.len() - 2);
                before.push('\n');
                // **`{empty}` の後ろの空白は字です**(書く側が置いた印)。
                // 印だけ外して、空白はそのまま残します
                match l.trim_start().strip_prefix(EMPTY_PARA) {
                    Some(nokori) => before.push_str(nokori),
                    None => before.push_str(cont),
                }
                continue;
            }
            // 段落の切れ目の直後は、字を継ぎ足す空白を入れません
            if !before.ends_with("\n\n") {
                before.push_str(seam(before, cont));
            }
            before.push_str(cont);
        }
    }

    // **セルは流れで並びます**(本家の作法。2026-08-18 に直した)。
    // 1行に1セルずつ書いても、桁の数で行に切り分けられます。前は
    // 「1行 = 1行」だったので、そう書いた表が縦1列に潰れていました。
    // 桁の数は**最初の行のセルの数**で決めます(本家は `cols=` があれば
    // それを見ますが、うちは塊の属性をまだ読みません)
    let mut cells: Vec<Cellbox> = Vec::new();
    let mut first_row_cols = 0usize;
    for (li, l) in joined.iter().enumerate() {
        let mut restv: &str = l;
        let mut this_row_cols = 0usize;
        while !restv.is_empty() {
            let (vspan, after_v) = if let Some(r) = restv.strip_prefix('.') {
                let (n, r2) = take_num(r)?;
                let r3 = r2.strip_prefix('+').ok_or("縦結合は .N+ の形")?;
                (n, r3)
            } else {
                (0u8, restv)
            };
            let (hspan, after_h) = match take_num(after_v) {
                Ok((n, r2)) if r2.starts_with('+') => (n, &r2[1..]),
                _ => (0u8, after_v),
            };
            // **本家のセルの指定を読み飛ばします**(`h|` 見出し・`^|` 中央・
            // `a|` AsciiDoc として組む など)。うちが効かせるのは結合だけで、
            // 残りは字にせず、指定として捨てます
            let after_spec = skip_attrs(after_h);
            // `a|` は「中を AsciiDoc として組む」= **段落を複数持てる**セル。
            // 実物の様式では 395 セルのうち 63 セルがこれに当たります(2026-08-19)
            let asciidoc_cell = after_h[..after_h.len() - after_spec.len()].contains('a');
            let body = after_spec
                .strip_prefix('|')
                .ok_or_else(|| format!("表の行はセルごとに | で始める: {l}"))?;
            let end = next_cell_start(body);
            let (cell_text, restn) = body.split_at(end);
            let mut cb = Cellbox {
                col_span: hspan,
                v_merge: if vspan > 1 { VMerge::Start } else { VMerge::None },
                ..Default::default()
            };
            // 縦結合の残り行数は、後で桁に切るときに使う
            // **式は字のまま取ります**(太字の印として読まない)
            // **空の段落も残します。** 様式のセルは、書き込む余白として
            // 空の段落を持っていることがあります(実物で 59 セル)。
            // 末尾の改行がその段落を表すので、分ける前には落としません
            // **行末の ` +` は AsciiDoc の改行**です(2026-08-31)。
            // 段落の切れ目ではないので、1つの段落の中の改行に戻します。
            // 表計算のセルの「区　分 / Type」がこの形です
            let hodoita;
            let cell_text: &str = if cell_text.contains(" +\n") {
                hodoita = cell_text.replace(" +\n", "\n");
                &hodoita
            } else {
                cell_text
            };
            // **`{empty}` の後ろの空白は字です**(書く側が置いた印)。
            // 印だけ外します — `{empty}` だけのセルは空の段落なので触りません
            let hazushita;
            let (cell_text, atama_wo_nokosu, oshiri_wo_nokosu): (&str, bool, bool) = {
                let mut t = cell_text.trim_matches(|c: char| c == '\t' || c == '\r');
                let mut atama = false;
                let mut oshiri = false;
                if let Some(n) = t.trim_start().strip_prefix(EMPTY_PARA) {
                    if !n.is_empty() {
                        t = n;
                        atama = true;
                    }
                }
                if let Some(n) = t.trim_end().strip_suffix(EMPTY_PARA) {
                    if !n.is_empty() {
                        t = n;
                        oshiri = true;
                    }
                }
                hazushita = t.to_string();
                (&hazushita, atama, oshiri)
            };
            let raw = cell_text.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r');
            let para_text: Vec<&str> = if asciidoc_cell && raw.contains("\n\n") {
                raw.split("\n\n").map(trim_edges).collect()
            } else if atama_wo_nokosu || oshiri_wo_nokosu {
                // 印を外した側の空白は**字として残します**
                let mut t = cell_text;
                if !atama_wo_nokosu {
                    t = t.trim_start_matches([' ', '\t', '\r', '\n']);
                }
                if !oshiri_wo_nokosu {
                    t = t.trim_end_matches([' ', '\t', '\r', '\n']);
                }
                vec![t]
            } else {
                vec![trim_edges(cell_text)]
            };
            let mut paras = Vec::with_capacity(para_text.len());
            for p in para_text {
                paras.push(Paragraph {
                    // `{empty}` は**空の段落**(書いた側と同じ決め)
                    runs: if p == EMPTY_PARA {
                        Vec::new()
                    } else if is_formula_cell(p) {
                        // **式は字のまま取ります**(太字の印として読まない)。
                        // ただし逃がした縦棒だけは戻します — 書く側が
                        // `="A|B"` を `="A\|B"` にするので、そのままだと
                        // 逆斜線が式の中に残ります(2026-08-20)
                        vec![Run {
                            text: p.replace("\\|", "|"),
                            size_pt: None,
                            font: None,
                            fmt: CharFormat::default(),
                        }]
                    } else {
                        // `\|` はセルの区切りの逃がし。字は `|` です
                        parse_inline(&p.replace("\\|", "|"), doc, fresh_note)?
                    },
                    line_spacing: 1.0,
                    ..Default::default()
                });
            }
            // 縦結合の行数を持ち回る場所が無いので、印だけ立てて
            // 下の切り分けで数える(`.N+` は N-1 行ぶん下に伸びる)。
            // **頭の段落にだけ**立てます
            if vspan > 1 {
                if let Some(p) = paras.first_mut() {
                    p.indent = vspan - 1;
                }
            }
            cb.paragraphs = paras;
            this_row_cols += cb.span();
            cells.push(cb);
            restv = restn;
        }
        if li == 0 {
            first_row_cols = this_row_cols;
        }
    }
    let ncols = col_spec.filter(|n| *n > 0).unwrap_or(first_row_cols).max(1);

    // 桁の数で行に切る。縦結合が下の行の桁を占めるぶんも数える
    let mut vstarts: Vec<(usize, u8)> = Vec::new(); // (桁, 残り行数)
    let mut it = cells.into_iter().peekable();
    while it.peek().is_some() || !vstarts.is_empty() {
        let mut row: Vec<Cellbox> = Vec::new();
        let mut cols = 0usize;
        vstarts.sort_by_key(|x| x.0);
        let pending = vstarts.clone();
        let mut next_vstarts: Vec<(usize, u8)> = Vec::new();
        for (col, rest) in pending {
            // 上から伸びてきた分を、その桁に置く
            while cols < col {
                match it.next() {
                    Some(c) => {
                        cols += c.span();
                        row.push(c);
                    }
                    None => break,
                }
            }
            row.push(Cellbox { v_merge: VMerge::Continue, ..Default::default() });
            cols += 1;
            if rest > 1 {
                next_vstarts.push((col, rest - 1));
            }
        }
        while cols < ncols {
            // **入れると溢れるセルは、次の行の頭にします**(2026-08-31)。
            // 前は幅を見ずに入れていたので、11 セルの後ろに幅 10 のセルが来ると
            // 1行に 21 桁ぶん詰まり、そこから後ろが**1行ずつずれて**
            // いました(国税庁の酒税の表で 47 セル)
            if let Some(tsugi) = it.peek() {
                if cols > 0 && cols + tsugi.span() > ncols {
                    break;
                }
            }
            let Some(c) = it.next() else { break };
            let s = c.span();
            let vertical = c.paragraphs.first().map(|p| p.indent).unwrap_or(0);
            if vertical > 0 {
                next_vstarts.push((cols, vertical));
            }
            cols += s;
            row.push(c);
        }
        if row.is_empty() {
            break;
        }
        // 持ち回りに使った indent を消す(段落の字下げではない)
        for c in &mut row {
            for p in &mut c.paragraphs {
                p.indent = 0;
            }
        }
        t.rows.push(row);
        vstarts = next_vstarts;
    }
    Ok(t)
}

/// 行を継ぐときの継ぎ目。**日本語どうしは空白を入れず、欧文は入れます。**
///
/// AsciiDoc は続く行を1つの段落にします。日本語の文を行で折ったときに空白が
/// 入ると語の間が空いて見え、英語の文で空白を入れないと語がくっつきます
/// (2026-08-18、本家の手引きを読ませて `CSS.The build` が出た)。
fn seam(before: &str, after: &str) -> &'static str {
    let a = before.chars().next_back();
    let b = after.chars().next();
    let wide_char = |c: Option<char>| {
        c.is_some_and(|c| {
            matches!(c as u32,
                0x3000..=0x303F   // 約物
                | 0x3040..=0x30FF // かな
                | 0x4E00..=0x9FFF // 漢字
                | 0xFF00..=0xFFEF // 全角
            )
        })
    };
    if wide_char(a) || wide_char(b) { "" } else { " " }
}

/// セルの指定(揃え・見出し・種類)を読み飛ばす。`|` の手前まで返す。
///
/// 本家の形は `[N*][N+][.N+][<^>][a-z]|`。うちが効かせるのは結合(`N+`・`.N+`)
/// だけで、残りは**指定として捨てます**(字にはしません)。
fn skip_attrs(s: &str) -> &str {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' | b'^' | b'>' | b'.' | b'*' | b'+' => i += 1,
            b'0'..=b'9' => i += 1,
            // 種類は1字(a=AsciiDoc・h=見出し・m=等幅・s=太字・e=斜体・l=そのまま・d=既定)
            c if c.is_ascii_lowercase() && b.get(i + 1) == Some(&b'|') => i += 1,
            _ => break,
        }
    }
    &s[i..]
}

/// 前後の**半角の空き**だけを落とす。
///
/// `trim` は全角の空白(U+3000)まで落とします。日本語の様式では行頭の
/// 全角空白が**字下げ**なので、落とすと見た目が変わります
/// (実物の様式で2セルが当たりました。2026-08-19)。
fn trim_edges(s: &str) -> &str {
    s.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n')
}

/// その行の**最後のセル**が `a|`(AsciiDoc として組む)か。
///
/// 空行を段落の切れ目として残すのは、このセルの中だけです。表の見た目を
/// 整えるための空行まで段落にすると、ふつうの表が崩れます。
fn last_cell_is_adoc(s: &str) -> bool {
    cell_kind(s).unwrap_or(false)
}

/// その行の最後のセルが `a|` か。**セルの頭が1つも無ければ `None`**
/// (前の行の続きなので、呼ぶ側は前の判断をそのまま持ち越します)。
fn cell_kind(s: &str) -> Option<bool> {
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut a = None;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == b'|' {
            a = Some(false); // 指定なしのセル
            i += 1;
            continue;
        }
        // 指定つきの頭(`a|` `h|` `2+|`)。空白か行頭の後ろだけ見ます
        if (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\n') && b[i] != b' ' {
            let rest = &s[i..];
            let after = skip_attrs(rest);
            if after.starts_with('|') {
                let attrs = &rest[..rest.len() - after.len()];
                a = Some(attrs.contains('a'));
                i += attrs.len() + 1;
                continue;
            }
        }
        i += s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    a
}

/// その行がセルの指定つきで始まるか(`h|` `^|` `2+|` など)
fn cell_has_attrs(l: &str) -> bool {
    skip_attrs(l).starts_with('|') && !l.starts_with('|')
}

/// 次のセルの `|`(結合の頭書きも考慮)の位置。無ければ末尾
fn next_cell_start(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == b'|' {
            return i;
        }
        // **指定つきの頭**(`h|` `^|` `2+|` `.2+|`)。空白の後ろにあるときだけ
        // 指定と見ます — 字の途中の `a|` を頭と読まないためです
        // 段落の切れ目(`a|` のセルの中)の後ろも行頭として見ます
        if (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\n')
            && b[i] != b' '
            && skip_attrs(&s[i..]).starts_with('|')
        {
            return i;
        }
        i += s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    b.len()
}

fn take_num(s: &str) -> Result<(u8, &str), String> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return Err("数がありません".into());
    }
    Ok((s[..end].parse().map_err(|_| "数が読めません")?, &s[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 恒等の門番。**正規形どうしで write(parse(x)) == x**
    fn round_trip(src: &str) {
        let doc = parse(src).expect(src);
        let back = write(&doc);
        assert_eq!(back, src, "往復で崩れた");
    }

    /// **節ごとの用紙**(2026-09-02 発注者。asciidoctor-pdf の書き方に合わせた)。
    /// `[page-…]` と `<<<` で新しい節になり、頭の `:page-…:` が最初の節の用紙
    #[test]
    fn sections_carry_their_paper() {
        let src = ":page-size: A4\n:page-layout: portrait\n:page-margin: 20mm\n\n縦の本文です。\n\n[page-size=A3, page-layout=landscape, page-margin=15mm]\n<<<\n\n横の本文です。\n";
        let d = parse(src).unwrap();
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps.len(), 2);
        let sb = ps[0].sect.as_ref().expect("前の段落に節の印が無い");
        assert!(!sb.continuous, "改ページする節になっていない");
        assert_eq!((sb.page.w_mm, sb.page.h_mm, sb.page.left_mm), (210.0, 297.0, 20.0), "最初の節の用紙");
        let last = d.page.expect("最後の節の用紙が無い");
        assert_eq!((last.w_mm, last.h_mm, last.top_mm), (420.0, 297.0, 15.0), "横の A3 になっていない: {last:?}");
        assert_eq!(write(&d), src, "往復で変わった");
    }

    #[test]
    fn a_continuous_section_has_no_page_break_line() {
        let src = "本文です。\n\n[page-size=A4, page-layout=portrait, page-margin=[25mm, 15mm, 25mm, 15mm]]\n\n続きの本文です。\n";
        let d = parse(src).unwrap();
        let sb = d.paragraphs().next().unwrap().sect.as_ref().unwrap();
        assert!(sb.continuous, "続きの節になっていない");
        let last = d.page.unwrap();
        assert_eq!((last.top_mm, last.right_mm, last.bottom_mm, last.left_mm), (25.0, 15.0, 25.0, 15.0));
        assert_eq!(write(&d), ":page-size: A4\n:page-layout: portrait\n:page-margin: 20mm\n\n".to_string() + src, "往復で変わった");
    }

    #[test]
    fn a_section_after_a_table_gets_its_own_empty_paragraph() {
        let src = "|===\n|a |b\n|===\n\n[page-size=B5, page-layout=portrait, page-margin=20mm]\n<<<\n\n後の本文です。\n";
        let d = parse(src).unwrap();
        assert!(matches!(&d.blocks[1], Block::Para(p) if p.runs.is_empty() && p.sect.is_some()), "表の後に節の印の段落が無い: {:?}", d.blocks.len());
        assert_eq!(d.page.unwrap().w_mm, 182.0);
        let w = write(&d);
        assert!(w.contains("|===\n\n[page-size=B5"), "表の後の切れ目が書けていない:\n{w}");
    }

    #[test]
    fn unknown_paper_is_refused_by_line() {
        let e = parse("本文です。\n\n[page-size=Z9]\n<<<\n\n次です。\n").unwrap_err();
        assert!(e.contains("3 行目") && e.contains("知らない用紙"), "{e}");
        assert!(parse("本文です。\n\n[page-layout=sideways]\n").is_err());
        // page- を持たない指定の行は、今までどおり原文のまま持ち越す
        let d = parse("[source,python]\n----\nprint(1)\n----\n").unwrap();
        assert!(d.paragraphs().all(|p| p.sect.is_none()));
    }

    #[test]
    fn the_header_alone_sets_the_first_paper() {
        let d = parse(":page-size: Letter\n:page-layout: landscape\n\n本文です。\n").unwrap();
        let pg = d.page.unwrap();
        assert_eq!((pg.w_mm, pg.h_mm), (279.4, 215.9));
        // 節が無ければ頭はそのまま(値も順も変えない)
        assert_eq!(write(&d), ":page-size: Letter\n:page-layout: landscape\n\n本文です。\n");
        assert!(parse("本文です。\n").unwrap().page.is_none(), "何も書いていないのに用紙を決めた");
    }

    /// **編集できるようにした書き方**(2026-08-18)。原文のまま持ち越すのを
    /// やめ、意味として読んで書き戻す形にしたので、往復で崩れないことを見る
    #[test]
    fn heading_4_and_5_round_trip() {
        round_trip("= 題\n\n===== 四段目\n\n====== 五段目\n");
        let doc = parse("===== 四段目\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.style, ParaStyle::Heading(4));
        assert_eq!(p.runs[0].text, "四段目", "印が字に残っている");
    }

    #[test]
    fn monospace_round_trips() {
        // 後ろが字なので二重の印(本家は `\`字\`と` を等幅として読まない)
        round_trip("``等幅の字``と普通の字。\n");
        round_trip("等幅は `これ` です。\n");
        let doc = parse("``等幅の字``と普通の字。\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.runs[0].fmt.style_id.as_deref(), Some(MONO));
        assert_eq!(p.runs[0].text, "等幅の字", "印が字に残っている");
        // **対でない印は書式にしない** — 後ろが全部等幅になるのを防ぐ
        let doc = parse("7`は素数ではありません。\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert!(p.runs.iter().all(|r| r.fmt.style_id.is_none()), "片方だけの ` を書式にした");
    }

    #[test]
    fn admonitions_round_trip() {
        round_trip("NOTE: 気をつけて。\n\nWARNING: 危ない。\n\nTIP: こつです。\n");
        let doc = parse("WARNING: 危ない。\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.style_id.as_deref(), Some("警告"));
        assert_eq!(p.runs[0].text, "危ない。", "印が字に残っている");
    }

    #[test]
    fn a_labelled_list_round_trips() {
        // 続いている間は空行で割らない(1つの一覧)
        round_trip("項目:: その説明\n別の項目:: 別の説明\n");
        let doc = parse("項目:: その説明\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.style_id.as_deref(), Some("説明のリスト"));
    }

    #[test]
    fn table_column_specs_round_trip() {
        round_trip("[cols=\"1,3\"]\n|===\n|狭い |広い\n|あ |い\n|===\n");
        let doc = parse("[cols=\"1,3\"]\n|===\n|あ|い\n|===\n").unwrap();
        let t = doc.tables().next().unwrap();
        assert_eq!(t.col_ratio, vec![1.0, 3.0]);
        // 比のまま持つ。mm になるのはテンプレートを合成するとき
        assert!(t.col_mm.is_empty(), "読んだ時点で mm を決めてしまった");
    }

    #[test]
    fn a_block_attribute_line_stays_with_its_block() {
        // 前は空行が入り、指定が塊に掛からなくなっていた
        round_trip("[source,python]\n----\nprint(1)\n----\n");
    }

    /// **一文一行**(2026-08-18)。git の差分が文ごとになる
    #[test]
    fn the_body_is_written_one_sentence_per_line() {
        let doc = parse("一つ目です。二つ目です。\n").unwrap();
        assert_eq!(write(&doc), "一つ目です。\n二つ目です。\n");
        // 読むと1つの段落に戻る(和字の継ぎ目に空白は入れない)
        round_trip("一つ目です。\n二つ目です。\n");
    }

    #[test]
    fn latin_text_breaks_only_at_spaces_and_capitals() {
        // 略語と頭文字では切らない
        round_trip("Dr. Smith went home.\nThe next one starts here.\n");
        let doc = parse("See example.com/a.b for more.\n").unwrap();
        assert_eq!(write(&doc), "See example.com/a.b for more.\n");
    }

    #[test]
    fn no_line_break_inside_a_box() {
        // 脚注の中の `。` と、等幅の中の `。`
        round_trip("脚注つきです。\nfootnote:[注の中の文。切りません]続きです。\n");
        round_trip("`コードの中。切りません` と 普通の文。\n");
    }

    #[test]
    fn headings_and_bullets_are_not_broken() {
        // 切ると、続く行が別の段落になってしまう
        round_trip("== 見出し。二文目。\n\n* 一つ。二つ。\n");
    }

    #[test]
    fn a_user_named_paragraph_can_span_any_number_of_lines() {
        // 註記とラベル付きリストだけが1行で1つ。名前つきの段落は普通の段落
        round_trip("[.強調の囲み]\n一つ目です。\n二つ目です。\n");
    }

    /// **表の題は表の物**(2026-08-18)。calc のシート名になり、式の中では
    /// 表の名前になる(`=SUM(売上台帳[金額])`)
    #[test]
    fn a_table_title_goes_into_the_table_and_round_trips() {
        round_trip(".売上台帳\n|===\n|品名 |金額\n\n|ペン |100\n|===\n");
        let (doc, ledger) = parse_full(".売上台帳\n|===\n|品名 |金額\n\n|ペン |100\n|===\n")
            .expect("読めない");
        let t = doc.tables().next().expect("表が無い");
        assert_eq!(t.title.as_deref(), Some("売上台帳"));
        assert!(t.header_row, "見出しの行が落ちた");
        // **取り込んだので帳簿には出さない**(出すと嘘になる)
        assert!(!ledger.iter().any(|x| x.contains("塊の題")), "{ledger:?}");
        // 表と関係のない `.題` は、いままでどおり原文のまま持ち越す
        let (_, ledger2) = parse_full(".ただの題\n\n本文。\n").expect("読めない");
        assert!(ledger2.iter().any(|x| x.contains("塊の題")), "{ledger2:?}");
    }

    #[test]
    fn nested_bullets_round_trip() {
        round_trip("* 一段目\n** 二段目\n*** 三段目\n* また一段目\n");
        round_trip(". 一つ目\n.. 中の一つ目\n");
        let doc = parse("** 二段目\n").unwrap();
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.list, ListKind::Bullet);
        assert_eq!(p.indent, 1);
        assert_eq!(p.runs[0].text, "二段目");
    }

    /// **塊の中身は種類の名前を持つ**(2026-09-03)。区切りの印と、直前の
    /// `[…]` の指定で決まる。書き戻しは字のまま(往復は変わらない)
    #[test]
    fn block_bodies_carry_their_kind() {
        let src = "[source,python]\n----\nprint(1)\n----\n\n....\nそのまま\n....\n\n[NOTE]\n====\n註記の中\n====\n\n====\n例の中\n====\n\n****\n傍注の中\n****\n\n////\n見えない\n////\n\n[verse]\n--\n詩の行\n--\n\n++++\n<b>生</b>\n++++\n";
        let d = parse(src).unwrap();
        let kinds: Vec<(String, String)> = d
            .paragraphs()
            .filter(|p| p.style_id.as_deref().is_some_and(is_block_inner))
            .map(|p| (p.style_id.clone().unwrap(), p.runs[0].text.clone()))
            .collect();
        assert_eq!(kinds, vec![
            ("コードの塊".to_string(), "print(1)".to_string()),
            ("字のまま出す塊".to_string(), "そのまま".to_string()),
            ("註記の塊".to_string(), "註記の中".to_string()),
            ("例の塊".to_string(), "例の中".to_string()),
            ("傍注の塊".to_string(), "傍注の中".to_string()),
            ("覚え書きの塊".to_string(), "見えない".to_string()),
            ("詩の塊".to_string(), "詩の行".to_string()),
            ("そのまま通す塊".to_string(), "<b>生</b>".to_string()),
        ]);
        assert_eq!(write(&d), src, "往復で変わった");
        assert!(is_mono_block("コードの塊") && is_hidden_block("覚え書きの塊"));
    }

    /// 塊の題と字下げの段落(2026-09-03)。題は `.` を取った字で持ち、原文はそのまま。
    /// 表の前の題は表に入る。字下げは等幅
    #[test]
    fn block_titles_show_without_the_dot_and_literal_paragraphs_are_mono() {
        let src = ".コードの題\n----\nx\n----\n\n.表の題\n|===\n|a\n|===\n\n  字下げの段落\n";
        let d = parse(src).unwrap();
        let title = d.paragraphs().find(|p| p.style_id.as_deref() == Some("塊の題")).expect("題が無い");
        assert_eq!(title.runs[0].text, "コードの題");
        assert_eq!(title.raw_adoc.as_deref(), Some(".コードの題"));
        assert_eq!(d.tables().next().unwrap().title.as_deref(), Some("表の題"), "表の題が取り込まれていない");
        assert!(d.paragraphs().any(|p| p.style_id.as_deref() == Some("字下げ")));
        assert!(is_mono_block("字下げ"));
        assert_eq!(write(&d), src, "往復で変わった");
    }

    #[test]
    fn the_body_of_a_code_block_can_be_edited() {
        // 塊の中の `*` は太字の印ではない。空行も残る
        round_trip("[source,python]\n----\nprint(\"*ほし*\")\n\nprint(1)\n----\n");
        let doc = parse("----\nprint(1)\n----\n").unwrap();
        let inner: Vec<&Paragraph> = doc
            .paragraphs()
            .filter(|p| p.style_id.as_deref() == Some("コードの塊"))
            .collect();
        assert_eq!(inner.len(), 1);
        assert!(inner[0].raw_adoc.is_none(), "字のまま持っていて直せない");
        assert_eq!(inner[0].runs[0].text, "print(1)");
    }

    #[test]
    fn headings_body_and_lists_round_trip() {
        round_trip("= 月次報告\n:template: 社内標準\n\n== まとめ\n\n売上は前月比で伸びた。\n\n* 東京\n* 大阪\n\n. 一番\n. 二番\n");
    }

    #[test]
    fn emphasis_and_quotes_round_trip() {
        // **囲みの外が字なので二重の印。** 本家は `*要点*だけ` を強調として
        // 読まない(2026-08-18 に本家で確かめた)
        round_trip("**要点**だけ__斜めに__言う。\n\n____\n引用の文。\n____\n");
    }

    #[test]
    fn footnotes_ruby_refs_and_bookmarks_round_trip() {
        round_trip("[[序]]\n本文footnote:[注の中身]の続きruby:漢字[かんじ]まで。\n\n<<序>>を見よ。\n");
    }

    #[test]
    fn links_formulas_images_and_page_breaks_round_trip() {
        round_trip("https://example.jp[例のサイト]を見る。\n\nstem:[x^2 + y^2 = 1]\n\nimage::images/図1.png[]\n\n<<<\n\n次の頁の文。\n");
    }

    #[test]
    fn a_table_round_trips_with_its_merges() {
        round_trip("|===\n|品 |数\n2+|合計だけの行\n|===\n");
    }

    #[test]
    fn the_reading_carries_meaning_only() {
        let d = parse("== 見出し\n\n**太い**字。\n").unwrap();
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[0].style, ParaStyle::Heading(1));
        // 見た目の欄は触らない — 大きさも書体も色も無指定のまま
        for p in &ps {
            for r in &p.runs {
                assert_eq!(r.size_pt, None);
                assert_eq!(r.font, None);
                assert_eq!(r.fmt.color, None);
            }
        }
        assert!(ps[1].runs[0].fmt.bold, "強調は意味なので入る");
    }

    #[test]
    fn an_escaped_mark_stays_as_text() {
        // **`\` は、印として読まれる所にだけ効きます**(本家の決め)。
        // `\*印*` の `*` は対になっていて印として読まれるので、`\` が
        // それを止めて字にします。書き戻せば同じ字に戻ります
        let d = parse("星は \\*印* と書く。\n").unwrap();
        let p: Vec<&Paragraph> = d.paragraphs().collect();
        let text: String = p[0].runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, "星は *印* と書く。");
        assert!(!p[0].runs.iter().any(|r| r.fmt.bold));
        assert_eq!(write(&d), "星は \\*印* と書く。\n");
        // 対の無い `*` は印にならないので、`\` も要らない(付けると見える)
        let d = parse("5 * 3 です。\n").unwrap();
        assert_eq!(write(&d), "5 * 3 です。\n");
    }

    /// 見出しの次の行は継がない。頭の覚え書きと本文の属性の項目は原文のまま
    #[test]
    fn a_heading_does_not_absorb_the_next_line() {
        let src = "// tag::title[]\n= 題\n:url-x: https://example.jp\n// end::title[]\n\n== 見出し\n本文の1行目。\n\n:page-x: 値\n\n本文。\n";
        let d = parse(src).unwrap();
        assert_eq!(d.attrs.iter().find(|(k, _)| k == "url-x").map(|(_, v)| v.as_str()), Some("https://example.jp"));
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        let heading = ps.iter().find(|p| p.style == ParaStyle::Heading(1)).expect("見出し");
        assert_eq!(heading.runs[0].text, "見出し", "見出しに次の行が混ざった");
        assert!(ps.iter().any(|p| p.raw_adoc.as_deref().map(str::trim_end) == Some(":page-x: 値")), "属性の項目が原文で残らない");
        // 書き戻しの正規形: 頭の覚え書きは題の後ろ、見出しの後ろは空行
        let want = src.replace("// tag::title[]\n= 題\n", "= 題\n// tag::title[]\n").replace("== 見出し\n", "== 見出し\n\n");
        assert_eq!(write(&d), want);
    }

    /// 行の中の `image:` の属性にある URL はリンクにしない(本家は画像を先に読む)
    #[test]
    fn a_url_inside_an_inline_image_macro_is_not_a_link() {
        let src = "image:a.jpg[A,100,,link=\"https://example.jp/a.jpg\"] と字。\n";
        let d = parse(src).unwrap();
        let p = d.paragraphs().next().unwrap();
        assert!(p.runs.iter().all(|r| r.fmt.link.is_none()), "画像の属性の URL がリンクになった");
        assert_eq!(write(&d), src);
    }

    #[test]
    fn superscript_and_subscript_round_trip() {
        round_trip("水は H^2^O ではなく H~2~O。\n");
    }

    #[test]
    fn character_level_styles_round_trip() {
        round_trip("ここは[.注意]#気をつける#ところ。\n");
        // **普通の文の `[.` は逃がしません**(2026-08-18)。逃がすのは
        // `[.名前]#` の形だけです。本家には `[.path]_径路_` のような役割の
        // 書き方があり、一律に逃がすと `\\` が入って別物になります
        round_trip("配列は [.5] と書く。\n");
        round_trip("径路は [.path]_data/x_ です。\n");
    }

    #[test]
    fn paragraph_style_names_round_trip() {
        round_trip("[.注意書き]\nここは気をつける。\n\nふつうの段落。\n");
    }

    /// **普通の AsciiDoc の文書が開ける。** 頭の属性(`:author:` など)は
    /// AsciiDoc の作法で、知らない名前だからと読むのをやめては、ただの
    /// AsciiDoc が開けないアプリになります(2026-08-18 に直しました)。
    /// 知らない名前も**持ち越して往復します** — 開いて保存しただけで
    /// 書いた人の字が消えないためです。
    #[test]
    fn unknown_attributes_are_carried_and_round_trip() {
        let src = "= 月次報告\n:author: 山田太郎\n:revdate: 2026-08-18\n:template: 社内標準\n\n                   == まとめ\n\n本文です。\n";
        let d = parse(src).expect("普通の AsciiDoc が読めない");
        assert_eq!(d.template.as_deref(), Some("社内標準"));
        assert_eq!(d.attrs.len(), 3, "属性を落とした: {:?}", d.attrs);
        assert_eq!(write(&d), src, "往復していない");
    }
}

/// **作業のリストの行か。**
///
/// `*` か `-` を段の数だけ並べ、`[ ]` か `[x]` が続く形です。
/// `* [ ]` `** [x]` `- [ ]` のどれも作業のリストです。
pub(crate) fn is_task_list(t: &str) -> bool {
    let mark = t.chars().next().filter(|c| *c == '*' || *c == '-');
    let Some(mark) = mark else { return false };
    let rest = t.trim_start_matches(mark);
    if rest.len() == t.len() {
        return false;
    }
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    rest.starts_with("[ ]") || rest.starts_with("[x]") || rest.starts_with("[X]")
}

/// **`-` の箇条書き**(2026-08-27)。AsciiDoc は `*` と `-` の両方を
/// 受けます。`-` を見ていなかったので、項目が1つの段落に潰れ、印も
/// 字のまま出ていました。実物の PDF を見て気づきました。
#[cfg(test)]
mod dash_bullet_tests {
    #[test]
    fn a_dash_list_becomes_separate_items() {
        let d = super::parse("本文。\n\n- 一つ目\n- 二つ目\n\n終わり。\n").expect("読めない");
        let items: Vec<String> = d
            .blocks
            .iter()
            .filter_map(|b| match b {
                crate::Block::Para(p) if p.list == crate::ListKind::Bullet => {
                    Some(p.runs.iter().map(|r| r.text.as_str()).collect())
                }
                _ => None,
            })
            .collect();
        assert_eq!(items, vec!["一つ目", "二つ目"], "項目に分かれていない");
    }

    /// **印は字として残りません。** `-` が本文に出ていました
    #[test]
    fn the_dash_is_not_left_in_the_text() {
        let d = super::parse("- 単価は税抜きです\n").expect("読めない");
        let t: String = d
            .blocks
            .iter()
            .filter_map(|b| match b {
                crate::Block::Para(p) => Some(p.runs.iter().map(|r| r.text.as_str()).collect::<String>()),
                _ => None,
            })
            .collect();
        assert!(!t.contains('-'), "印が字として残っている: {t:?}");
    }

    /// **`--` は塊の囲み**です。箇条書きとして読むと囲みが壊れます
    #[test]
    fn a_double_dash_is_a_block_fence_not_a_bullet() {
        let d = super::parse("--\n囲みの中\n--\n").expect("読めない");
        let bullets = d
            .blocks
            .iter()
            .filter(|b| matches!(b, crate::Block::Para(p) if p.list == crate::ListKind::Bullet))
            .count();
        assert_eq!(bullets, 0, "囲みを箇条書きとして読んでいる");
    }
}
