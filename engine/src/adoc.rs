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
    Block, Cellbox, CharFormat, Document, Footnote, FootnoteRef, InlineImage, ListKind,
    Paragraph, ParaStyle, RefField, Run, Table, VMerge,
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
    if !doc.props.title.is_empty() {
        head.push_str(&format!("= {}\n", doc.props.title));
    }
    let 名 = |k: &str| k == "template" || k == "テンプレート";
    if let Some(t) = &doc.template {
        if !doc.attrs.iter().any(|(k, _)| 名(k)) {
            // このアプリが後から名前を付けた(読んだ字には無かった)
            head.push_str(&format!(":template: {t}\n"));
        }
    }
    for (k, v) in &doc.attrs {
        // テンプレートの名前は後から変わりうるので、いまの値で書きます
        let v = if 名(k) { doc.template.clone().unwrap_or_else(|| v.clone()) } else { v.clone() };
        head.push_str(&format!(":{k}: {v}\n"));
    }
    if !head.is_empty() {
        out.push_str(&head);
        out.push('\n');
    }
    let mut quote_open = false;
    for (bi, b) in doc.blocks.iter().enumerate() {
        match b {
            Block::Para(p) => {
                let is_quote = p.style == ParaStyle::Quote;
                if quote_open && !is_quote {
                    out.push_str("____\n\n");
                    quote_open = false;
                }
                if is_quote && !quote_open {
                    out.push_str("____\n");
                    quote_open = true;
                }
                // 同じ種類のリストが続く間は空行を挟まない(1つのリスト)
                let tight = p.list != ListKind::None
                    && matches!(
                        doc.blocks.get(bi + 1),
                        Some(Block::Para(q)) if q.list == p.list
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
        (_, ListKind::Bullet) => "* ".into(),
        (_, ListKind::Number) => ". ".into(),
        _ => String::new(),
    };
    out.push_str(&head);
    out.push_str(&runs_text(&p.runs, doc));
    out.push('\n');
    if !in_quote {
        out.push('\n');
    }
}

/// run の並び → インラインの印つきの1行
fn runs_text(runs: &[Run], doc: &Document) -> String {
    let mut s = String::new();
    let mut bold = false;
    let mut italic = false;
    for r in runs {
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
            // 参照。見えている値は写しでしかないので、的の名前だけ書く
            let _ = f.page;
            s.push_str(&format!("<<{}>>", f.name));
            continue;
        }
        // 強調の開閉(正規形: 閉じ忘れは write 側では起きない — run の境で必ず対にする)
        if r.fmt.bold != bold {
            s.push('*');
            bold = r.fmt.bold;
        }
        if r.fmt.italic != italic {
            s.push('_');
            italic = r.fmt.italic;
        }
        // 上付き・下付きは**意味**(x² / H₂O)。AsciiDoc の標準の印
        let (sup, sub) = (r.fmt.superscript, r.fmt.subscript);
        if sup {
            s.push('^');
        }
        if sub {
            s.push('~');
        }
        // **文字単位のスタイル**(2026-08-16)。AsciiDoc の役割の書き方
        // `[.名前]#字#`。段落のスタイルと同じ表(テンプレート)を引く
        let (open, close) = match &r.fmt.style_id {
            Some(n) => (format!("[.{n}]#"), "#"),
            None => (String::new(), ""),
        };
        s.push_str(&open);
        if let Some(sdt) = &r.fmt.sdt {
            s.push_str(&field_src(sdt));
        } else if let Some(ruby) = &r.fmt.ruby {
            s.push_str(&format!("ruby:{}[{}]", esc(&r.text), ruby));
        } else if let Some(url) = &r.fmt.link {
            s.push_str(&format!("{url}[{}]", esc(&r.text)));
        } else {
            s.push_str(&esc(&r.text));
        }
        s.push_str(close);
        if sub {
            s.push('~');
        }
        if sup {
            s.push('^');
        }
    }
    if bold {
        s.push('*');
    }
    if italic {
        s.push('_');
    }
    s
}

/// 本文の字の中の印を逃がす。逃がすのは**行の中で意味を持つ印だけ**
fn esc(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        if c == '*' || c == '_' || c == '^' || c == '~' || c == '\\' {
            s.push('\\');
        }
        // **`[.` だけ逃がす** — 文字スタイルの書き出しと紛れるのはこの形だけ。
        // `[` を一律に逃がすと、括弧つきの普通の文が読みにくくなる
        if c == '[' && it.peek() == Some(&'.') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

/// 記入欄の `[…]` の中身 → (表示名, 種類, 選択肢)。
///
/// 種類の名前は日本語で書きます。知らない名前は**文字の欄として扱い、
/// 黙って捨てません**(表示名の一部として残ります)。
fn parse_field(s: &str) -> (String, crate::doc::SdtKind, Vec<String>) {
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
        K::Date => o.push_str(",日付"),
        K::Email => o.push_str(",メール"),
        K::Phone => o.push_str(",電話"),
        K::Picture => o.push_str(",画像"),
        K::Signature => o.push_str(",署名"),
    }
    o.push(']');
    o
}

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
    out.push_str("|===\n");
    for (ri, row) in t.rows.iter().enumerate() {
        for (k, cell) in row.iter().enumerate() {
            match cell.v_merge {
                VMerge::Continue => {
                    // 縦結合の続き = セルを書かない(頭の .N+ が占める)
                    continue;
                }
                VMerge::Start => {
                    let n = vspan_of(t, ri, grid_col(row, k));
                    if n > 1 {
                        out.push_str(&format!(".{n}+"));
                    }
                }
                VMerge::None => {}
            }
            if cell.span() > 1 {
                out.push_str(&format!("{}+", cell.span()));
            }
            out.push('|');
            let text: String = cell
                .paragraphs
                .iter()
                .map(|p| runs_text(&p.runs, doc))
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&text);
        }
        out.push('\n');
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
    let mut 使った: Vec<String> = Vec::new();
    each_image(doc, &mut |im| {
        if let Some(s) = &im.src {
            使った.push(s.clone());
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
            if !使った.contains(&name) {
                break name;
            }
        };
        使った.push(name.clone());
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
    let mut 足す = |name: &'static str| {
        if !v.contains(&name) {
            v.push(name);
        }
    };
    if !doc.header.paragraphs.is_empty() {
        足す("ヘッダー");
    }
    if !doc.footer.paragraphs.is_empty() {
        足す("フッター");
    }
    if doc.watermark.is_some() {
        足す("透かし");
    }
    if doc.page_color.is_some() {
        足す("ページの色");
    }
    if doc.vertical {
        足す("縦書き");
    }
    if doc.page.map(|p| p.columns > 1).unwrap_or(false) {
        足す("段組み");
    }
    if !doc.ink.is_empty() {
        足す("手描きの線");
    }
    // **表の中の段落も見ます。** 事務の様式は中身が表の中にあるので、
    // 本文だけ見ると「何も落ちません」と嘘を言うことになります
    let 表の中 = doc.blocks.iter().filter_map(|b| match b {
        Block::Table(t) => Some(t),
        Block::Para(_) => None,
    });
    let cells = 表の中
        .flat_map(|t| t.rows.iter())
        .flat_map(|r| r.iter())
        .flat_map(|c| c.paragraphs.iter());
    for p in doc.paragraphs().chain(cells) {
        if !p.comments.is_empty() {
            足す("コメント");
        }
        if p.align != crate::doc::Align::Left {
            足す("段落の揃え");
        }
        if p.indent > 0 || p.first_line_twips != 0 {
            足す("字下げ");
        }
        if p.line_spacing > 0.0 && p.line_spacing != 1.0 {
            足す("行間");
        }
        if p.space_before_pt != 0.0 || p.space_after_pt != 0.0 {
            足す("段落の前後の空き");
        }
        if p.shade.is_some() {
            足す("段落の背景色");
        }
        if p.boxed {
            足す("段落の囲み");
        }
        if p.dropcap {
            足す("ドロップキャップ");
        }
        if matches!(p.style, ParaStyle::Toc(_) | ParaStyle::Tof) {
            足す("目次の印");
        }
        for r in &p.runs {
            // 相互参照は「しおりの文字」と「ページ番号」の2種類だが、adoc は
            // `<<名前>>` の1種類しか持たない。ページ番号は文字の参照になる
            if r.fmt.field.as_ref().is_some_and(|f| f.page) {
                足す("相互参照のページ番号");
            }
            if r.fmt.underline {
                足す("下線");
            }
            if r.fmt.strike {
                足す("取り消し線");
            }
            if r.fmt.color.is_some() {
                足す("文字の色");
            }
            if r.fmt.highlight.is_some() {
                足す("背景の色");
            }
            if r.size_pt.is_some() {
                足す("字の大きさ");
            }
            if r.font.is_some() {
                足す("フォント");
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
fn 本家だけの書き方(l: &str) -> Option<&'static str> {
    let t = l.trim_start();
    for 印 in ["NOTE: ", "TIP: ", "IMPORTANT: ", "WARNING: ", "CAUTION: "] {
        if t.starts_with(印) {
            return Some("註記(NOTE: など)");
        }
    }
    // 塊の区切り。4つ以上の同じ記号だけの行
    for 印 in ['-', '.', '*', '_', '='] {
        if t.len() >= 4 && t.chars().all(|c| c == 印) && 印 != '_' {
            return Some("塊の区切り(---- など)");
        }
    }
    if t.starts_with("include::") {
        return Some("取り込み(include::)");
    }
    if t.starts_with("=====") {
        return Some("4段より深い見出し");
    }
    if t.starts_with("* [x]") || t.starts_with("* [ ]") || t.starts_with("- [x]") {
        return Some("チェックの箇条書き");
    }
    // 説明のリスト(用語:: 説明)。マクロ(名前:的[…])と紛れないよう `:: ` を見る
    if let Some(i) = t.find(":: ") {
        if i > 0 && !t[..i].contains(' ') && !t[..i].contains('[') {
            return Some("説明のリスト(用語:: 説明)");
        }
    }
    // 塊の題(.題)。箇条書きの `. ` とは違う
    if t.starts_with('.') && !t.starts_with(". ") && t.len() > 1 && !t.starts_with("..") {
        return Some("塊の題(.題)");
    }
    // 属性の参照 {名前}。うちの差し込みは {{名前}} なので、二重は数えない
    let b = t.as_bytes();
    for (i, c) in t.char_indices() {
        // **うちの差し込みは `{{名前}}`** なので、二重の中括弧は数えない
        let 二重 = b.get(i + 1) == Some(&b'{') || (i > 0 && b[i - 1] == b'{');
        if c == '{' && !二重 && t[i..].contains('}') {
            return Some("属性の参照({名前})");
        }
    }
    None
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
    let mut 帳簿: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut doc = Document::default();
    let mut lines = src.lines().enumerate().peekable();
    let mut pending_bookmarks: Vec<String> = Vec::new();
    let mut pending_break = false;
    let mut pending_style: Option<String> = None;
    let mut in_quote = false;
    let mut fresh_note = 0usize;

    // 文書の頭: `= 題名` と `:鍵: 値`
    let mut head_done = false;
    while let Some((_, line)) = lines.peek().copied() {
        let l = line.trim_end();
        if !head_done && doc.props.title.is_empty() && l.starts_with("= ") {
            doc.props.title = l[2..].trim().to_string();
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
                    lines.next();
                    continue;
                }
            }
            if l.is_empty() {
                head_done = true;
                lines.next();
                continue;
            }
            // 頭の印なしで本文が始まる形も受ける(下の break でそのまま本文へ)
        }
        break;
    }

    while let Some((ln, line)) = lines.next() {
        let l = line.trim_end();
        if l.is_empty() {
            continue;
        }
        if let Some(何) = 本家だけの書き方(l) {
            *帳簿.entry(何).or_default() += 1;
        }
        if l == "____" {
            in_quote = !in_quote;
            continue;
        }
        if l == "<<<" {
            pending_break = true;
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
            for (_, tl) in lines.by_ref() {
                let tl = tl.trim_end();
                if tl == "|===" {
                    closed = true;
                    break;
                }
                if !tl.is_empty() {
                    rows.push(tl);
                }
            }
            if !closed {
                return Err(format!("{} 行目: |=== が閉じていません", ln + 1));
            }
            let t = parse_table_lines(&rows, &mut doc, &mut fresh_note)?;
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
        } else if let Some(rest) = l.strip_prefix("* ") {
            p.list = ListKind::Bullet;
            rest
        } else if let Some(rest) = l.strip_prefix(". ") {
            p.list = ListKind::Number;
            rest
        } else {
            l
        };
        if in_quote {
            p.style = ParaStyle::Quote;
        }
        p.runs = parse_inline(body, &mut doc, &mut fresh_note)?;
        doc.blocks.push(Block::Para(p));
    }
    let 帳簿 = 帳簿
        .into_iter()
        .map(|(k, n)| if n > 1 { format!("{k} × {n}") } else { k.to_string() })
        .collect();
    Ok((doc, 帳簿))
}

fn base_para(
    bookmarks: &mut Vec<String>,
    brk: &mut bool,
    style: &mut Option<String>,
) -> Paragraph {
    let mut p = Paragraph::default();
    p.bookmarks = std::mem::take(bookmarks);
    p.page_break_before = std::mem::take(brk);
    p.style_id = style.take();
    p
}

/// `== 見出し` → (1, "見出し")。`=` の数 − 1 が水準(1〜3)
fn heading_of(l: &str) -> Option<(u8, &str)> {
    for n in (1..=3u8).rev() {
        let mark = "=".repeat(n as usize + 1) + " ";
        if let Some(rest) = l.strip_prefix(&mark) {
            return Some((n, rest));
        }
    }
    None
}

/// `path[attrs]` を (path, attrs) に割る
fn split_macro_target(s: &str) -> Option<(&str, &str)> {
    let open = s.find('[')?;
    let close = s.rfind(']')?;
    (close == s.len() - 1).then(|| (&s[..open], &s[open + 1..close]))
}

/// インラインの印を run の並びへ。
/// `*太*` `_斜_` `ruby:字[よみ]` `footnote:[中身]` `<<参照>>` `https://…[名]` `\逃がし`
///
/// **添字は全部バイトで数える。** 字数と混ぜると、日本語の後ろの字が
/// 食われる(最初の版で `の続き` が消えた — find の返りはバイト)
fn parse_inline(
    text: &str,
    doc: &mut Document,
    fresh_note: &mut usize,
) -> Result<Vec<Run>, String> {
    let mut runs: Vec<Run> = Vec::new();
    let mut cur = String::new();
    let mut bold = false;
    let mut italic = false;
    let flush = |runs: &mut Vec<Run>, cur: &mut String, bold: bool, italic: bool| {
        if cur.is_empty() {
            return;
        }
        let mut fmt = CharFormat::default();
        fmt.bold = bold;
        fmt.italic = italic;
        runs.push(Run { text: std::mem::take(cur), size_pt: None, font: None, fmt });
    };
    let mut i = 0usize; // バイト
    while i < text.len() {
        let rest = &text[i..];
        if let Some(after) = rest.strip_prefix('\\') {
            if let Some(c) = after.chars().next() {
                cur.push(c);
                i += 1 + c.len_utf8();
                continue;
            }
        }
        if rest.starts_with('*') {
            flush(&mut runs, &mut cur, bold, italic);
            bold = !bold;
            i += 1;
            continue;
        }
        if rest.starts_with('_') {
            flush(&mut runs, &mut cur, bold, italic);
            italic = !italic;
            i += 1;
            continue;
        }
        if let Some(after) = rest.strip_prefix("[.") {
            if let (Some(rb), Some(_)) = (after.find("]#"), after.find('#')) {
                let name = &after[..rb];
                if !name.is_empty() && !name.contains(['[', ']', '#', ' ']) {
                    let body = &after[rb + 2..];
                    if let Some(end) = body.find('#') {
                        flush(&mut runs, &mut cur, bold, italic);
                        let mut fmt = CharFormat::default();
                        fmt.bold = bold;
                        fmt.italic = italic;
                        fmt.style_id = Some(name.to_string());
                        runs.push(Run {
                            text: body[..end].to_string(),
                            size_pt: None,
                            font: None,
                            fmt,
                        });
                        i += 2 + rb + 2 + end + 1;
                        continue;
                    }
                }
            }
        }
        if rest.starts_with('^') || rest.starts_with('~') {
            let up = rest.starts_with('^');
            let close = if up { '^' } else { '~' };
            if let Some(end) = rest[1..].find(close) {
                flush(&mut runs, &mut cur, bold, italic);
                let mut fmt = CharFormat::default();
                fmt.bold = bold;
                fmt.italic = italic;
                fmt.superscript = up;
                fmt.subscript = !up;
                runs.push(Run {
                    text: rest[1..1 + end].to_string(),
                    size_pt: None,
                    font: None,
                    fmt,
                });
                i += 1 + end + 1;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("<<") {
            if let Some(end) = after.find(">>") {
                flush(&mut runs, &mut cur, bold, italic);
                let name = after[..end].to_string();
                let mut fmt = CharFormat::default();
                fmt.bold = bold;
                fmt.italic = italic;
                fmt.field = Some(RefField { name: name.clone(), page: false });
                runs.push(Run { text: name, size_pt: None, font: None, fmt });
                i += 2 + end + 2;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("footnote:[") {
            let end = after.find(']').ok_or("footnote:[ が閉じていません")?;
            flush(&mut runs, &mut cur, bold, italic);
            *fresh_note += 1;
            let id = format!("adoc{fresh_note}");
            let mut np = Paragraph::default();
            np.runs = vec![Run {
                text: after[..end].to_string(),
                size_pt: None,
                font: None,
                fmt: CharFormat::default(),
            }];
            doc.footnotes.push(Footnote {
                id: id.clone(),
                endnote: false,
                paragraphs: vec![np],
                added: true,
            });
            let mut fmt = CharFormat::default();
            fmt.footnote = Some(FootnoteRef { id, endnote: false });
            runs.push(Run { text: String::new(), size_pt: None, font: None, fmt });
            i += "footnote:[".len() + end + 1;
            continue;
        }
        // 記入欄。`field:タグ[表示名]` / `field:タグ[表示名,種類]` /
        // `field:タグ[表示名,選ぶ:一般|学生]`
        //
        // **記入欄は意味です。** 「ここに名前を書く」という指示であって
        // 見た目ではないので、意味だけの本文に書けます(2026-08-17。
        // アプリの形で書き出すときの土台)。
        if let Some(after) = rest.strip_prefix("field:") {
            if let Some(open) = after.find('[') {
                if let Some(close) = after[open..].find(']') {
                    flush(&mut runs, &mut cur, bold, italic);
                    let tag = after[..open].to_string();
                    let 中 = &after[open + 1..open + close];
                    let (alias, kind, items) = parse_field(中);
                    let mut fmt = CharFormat::default();
                    fmt.sdt = Some(Box::new(crate::doc::Sdt { kind, alias, tag, items }));
                    runs.push(Run { text: String::new(), size_pt: None, font: None, fmt });
                    i += "field:".len() + open + close + 1;
                    continue;
                }
            }
        }
        if let Some(after) = rest.strip_prefix("ruby:") {
            if let Some(open) = after.find('[') {
                if let Some(close) = after[open..].find(']') {
                    flush(&mut runs, &mut cur, bold, italic);
                    let mut fmt = CharFormat::default();
                    fmt.bold = bold;
                    fmt.italic = italic;
                    fmt.ruby = Some(after[open + 1..open + close].to_string());
                    runs.push(Run {
                        text: after[..open].to_string(),
                        size_pt: None,
                        font: None,
                        fmt,
                    });
                    i += "ruby:".len() + open + close + 1;
                    continue;
                }
            }
        }
        if rest.starts_with("https://") || rest.starts_with("http://") {
            if let Some(open) = rest.find('[') {
                if let Some(close) = rest[open..].find(']') {
                    let url = &rest[..open];
                    if !url.contains(' ') {
                        flush(&mut runs, &mut cur, bold, italic);
                        let mut fmt = CharFormat::default();
                        fmt.bold = bold;
                        fmt.italic = italic;
                        fmt.link = Some(url.to_string());
                        runs.push(Run {
                            text: rest[open + 1..open + close].to_string(),
                            size_pt: None,
                            font: None,
                            fmt,
                        });
                        i += open + close + 1;
                        continue;
                    }
                }
            }
        }
        let c = rest.chars().next().unwrap();
        cur.push(c);
        i += c.len_utf8();
    }
    flush(&mut runs, &mut cur, bold, italic);
    Ok(runs)
}

fn parse_table_lines(
    rows_src: &[&str],
    doc: &mut Document,
    fresh_note: &mut usize,
) -> Result<Table, String> {
    let mut t = Table::default();
    // 縦結合の続き(列, 残り行数)。docx の格子と同じく Continue を配る
    let mut vstarts: Vec<(usize, u8)> = Vec::new();
    for l in rows_src {
        let mut row: Vec<Cellbox> = Vec::new();
        // 前の行から続く縦結合ぶんの Continue を先に置く
        vstarts.sort_by_key(|x| x.0);
        let pending = vstarts.clone();
        for &(col, _) in &pending {
            let mut at = row.len();
            let mut acc = 0usize;
            for (k, c) in row.iter().enumerate() {
                if acc >= col {
                    at = k;
                    break;
                }
                acc += c.span();
            }
            let mut cb = Cellbox::default();
            cb.v_merge = VMerge::Continue;
            row.insert(at.min(row.len()), cb);
        }
        for s in &mut vstarts {
            s.1 -= 1;
        }
        vstarts.retain(|s| s.1 > 0);
        // 頭の結合の印つきでセルを割る: [.N+][M+]|中身
        let mut restv: &str = l;
        while !restv.is_empty() {
            let (vspan, after_v) = if let Some(r) = restv.strip_prefix('.') {
                let (n, r2) = take_num(r)?;
                let r3 = r2
                    .strip_prefix('+')
                    .ok_or("縦結合は .N+ の形")?;
                (n, r3)
            } else {
                (0u8, restv)
            };
            let (hspan, after_h) = match take_num(after_v) {
                Ok((n, r2)) if r2.starts_with('+') => (n, &r2[1..]),
                _ => (0u8, after_v),
            };
            let body = after_h
                .strip_prefix('|')
                .ok_or_else(|| format!("表の行はセルごとに | で始める: {l}"))?;
            let end = next_cell_start(body);
            let (cell_text, restn) = body.split_at(end);
            let mut cb = Cellbox::default();
            cb.col_span = hspan;
            cb.v_merge = if vspan > 1 { VMerge::Start } else { VMerge::None };
            if vspan > 1 {
                vstarts.push((row.iter().map(|c: &Cellbox| c.span()).sum::<usize>(), vspan - 1));
            }
            let mut p = Paragraph::default();
            p.runs = parse_inline(cell_text.trim(), doc, fresh_note)?;
            cb.paragraphs = vec![p];
            row.push(cb);
            restv = restn;
        }
        t.rows.push(row);
    }
    Ok(t)
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
        // `.2+|` / `3+|` の頭書きの前で切る
        let mut j = i;
        if b[j] == b'.' {
            j += 1;
        }
        let d0 = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > d0 && j < b.len() && b[j] == b'+' && j + 1 < b.len() && b[j + 1] == b'|' {
            return i;
        }
        i += s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    s.len()
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
    fn 往復(src: &str) {
        let doc = parse(src).expect(src);
        let back = write(&doc);
        assert_eq!(back, src, "往復で崩れた");
    }

    #[test]
    fn 見出しと本文とリストが往復する() {
        往復("= 月次報告\n:template: 社内標準\n\n== まとめ\n\n売上は前月比で伸びた。\n\n* 東京\n* 大阪\n\n. 一番\n. 二番\n");
    }

    #[test]
    fn 強調と引用が往復する() {
        往復("*要点*だけ_斜めに_言う。\n\n____\n引用の文。\n____\n");
    }

    #[test]
    fn 脚注とルビと参照としおりが往復する() {
        往復("[[序]]\n本文footnote:[注の中身]の続きruby:漢字[かんじ]まで。\n\n<<序>>を見よ。\n");
    }

    #[test]
    fn リンクと数式と画像と改ページが往復する() {
        往復("https://example.jp[例のサイト]を見る。\n\nstem:[x^2 + y^2 = 1]\n\nimage::images/図1.png[]\n\n<<<\n\n次の頁の文。\n");
    }

    #[test]
    fn 表が結合ごと往復する() {
        往復("|===\n|品|数\n2+|合計だけの行\n|===\n");
    }

    #[test]
    fn 読みは意味だけを入れる() {
        let d = parse("== 見出し\n\n*太い*字。\n").unwrap();
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
    fn 逃がした印は字として残る() {
        let d = parse("星は \\* と書く。\n").unwrap();
        let p: Vec<&Paragraph> = d.paragraphs().collect();
        let text: String = p[0].runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, "星は * と書く。");
        assert!(!p[0].runs.iter().any(|r| r.fmt.bold));
        // 書き戻せば逃がしも戻る
        assert_eq!(write(&d), "星は \\* と書く。\n");
    }

    #[test]
    fn 上付きと下付きが往復する() {
        往復("水は H^2^O ではなく H~2~O。\n");
    }

    #[test]
    fn 文字単位のスタイルが往復する() {
        往復("ここは[.注意]#気をつける#ところ。\n");
        // 普通の文の `[.` は逃がして残す
        往復("配列は \\[.5] と書く。\n");
    }

    #[test]
    fn 段落のスタイル名が往復する() {
        往復("[.注意書き]\nここは気をつける。\n\nふつうの段落。\n");
    }

    /// **普通の AsciiDoc の文書が開ける。** 頭の属性(`:author:` など)は
    /// AsciiDoc の作法で、知らない名前だからと読むのをやめては、ただの
    /// AsciiDoc が開けないアプリになります(2026-08-18 に直しました)。
    /// 知らない名前も**持ち越して往復します** — 開いて保存しただけで
    /// 書いた人の字が消えないためです。
    #[test]
    fn 知らない属性も持ち越して往復する() {
        let src = "= 月次報告\n:author: 山田太郎\n:revdate: 2026-08-18\n:template: 社内標準\n\n                   == まとめ\n\n本文です。\n";
        let d = parse(src).expect("普通の AsciiDoc が読めない");
        assert_eq!(d.template.as_deref(), Some("社内標準"));
        assert_eq!(d.attrs.len(), 3, "属性を落とした: {:?}", d.attrs);
        assert_eq!(write(&d), src, "往復していない");
    }
}
