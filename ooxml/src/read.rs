//! **docx を読む。** 解釈できなかった要素は捨てずに `Report` へ。

use std::io::{Read, Seek};

use kumihan::{Align, Block, Cellbox, CharFormat, Comment, Document, ListKind, ParaStyle,
              Paragraph, RefField, Run, Stroke, Table, VMerge, PAGES_MARK, PAGE_MARK};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::write::*;

/// 読み書きで落ちた・触れなかったものの記録。
/// 「読めたつもり」を防ぐための帳簿。
#[derive(Debug, Default, Clone)]
pub struct Report {
    /// 未対応で無視した要素(w:tbl など)と回数
    pub unsupported: Vec<(String, usize)>,
    /// 段落数・ラン数
    pub paragraphs: usize,
    pub runs: usize,
}

impl Report {
    fn note(&mut self, name: &str) {
        if let Some(e) = self.unsupported.iter_mut().find(|(n, _)| n == name) {
            e.1 += 1;
        } else {
            self.unsupported.push((name.to_string(), 1));
        }
    }
    pub fn is_lossless(&self) -> bool {
        self.unsupported.is_empty()
    }
}


/// `<w:b/>` は付ける、`<w:b w:val="0"/>` は付けない。
/// 有無だけで見ると「太字を解除した文書」を太字にしてしまう。
pub(super) fn on(e: &quick_xml::events::BytesStart) -> bool {
    !matches!(attr(e, "val").as_deref(), Some("0") | Some("false") | Some("none"))
}

pub(super) fn local(name: &[u8]) -> &[u8] {
    match name.iter().position(|b| *b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

pub(super) fn attr(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local(a.key.as_ref()) == want.as_bytes() {
            Some(String::from_utf8_lossy(&a.value).to_string())
        } else {
            None
        }
    })
}

// ---------- 読む ----------

/// docx を読む。返るのは文書と、読めなかったものの帳簿。
pub fn read<R: Read + Seek>(src: R) -> Result<(Document, Report), String> {
    let mut zip = zip::ZipArchive::new(src).map_err(|e| format!("zipを開けません: {e}"))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|_| "word/document.xml がありません(docxではない可能性)".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| format!("document.xml を読めません: {e}"))?;

    // 関係ID → 部品名 を先に引いておく。
    // rels: <Relationship Id="rId5" Target="media/image1.png"/>。
    // 画像の実体のほか、ヘッダー・フッターの部品名もここから引く
    let mut rels = String::new();
    if let Ok(mut f) = zip.by_name("word/_rels/document.xml.rels") {
        let _ = f.read_to_string(&mut rels);
    }
    let mut targets: std::collections::BTreeMap<String, String> = Default::default();
    let mut at = 0usize;
    while let Some(i) = rels[at..].find("Id=\"") {
        let s = at + i + 4;
        let Some(e) = rels[s..].find('"') else { break };
        let id = rels[s..s + e].to_string();
        // 同じ Relationship の中の Target を探す(次の > まで)
        let tail = &rels[s + e..];
        if let Some(ti) = tail.find("Target=\"") {
            let ts = ti + 8;
            if let Some(te) = tail[ts..].find('"') {
                targets.insert(id, tail[ts..ts + te].to_string());
            }
        }
        at = s + e;
    }
    let mut media: std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>> =
        Default::default();
    for (id, target) in &targets {
        if target.starts_with("media/") {
            let path = format!("word/{target}");
            if let Ok(mut mf) = zip.by_name(&path) {
                let mut buf = Vec::new();
                if mf.read_to_end(&mut buf).is_ok() {
                    media.insert(id.clone(), std::sync::Arc::new(buf));
                }
            }
        }
    }
    // 文書の情報(docProps/core.xml)。作成者・タイトルなど
    let mut pxml = String::new();
    if let Ok(mut f) = zip.by_name("docProps/core.xml") {
        let _ = f.read_to_string(&mut pxml);
    }

    // スタイル定義の名乗り(styles.xml の id・名前・種類)。
    // 定義の本体は保存で原本ごと持ち越す — ここは一覧を見せる写し
    // (2026-08-12 発注者確定「スタイル定義は持たない主義では無理」)
    let mut styxml = String::new();
    if let Ok(mut f) = zip.by_name("word/styles.xml") {
        let _ = f.read_to_string(&mut styxml);
    }

    // 設定(settings.xml)。欧文ハイフネーションの旗を読む
    let mut sxml = String::new();
    if let Ok(mut f) = zip.by_name("word/settings.xml") {
        let _ = f.read_to_string(&mut sxml);
    }

    // 脚注・文末脚注の中身。**紙面に出すためだけに読む** —
    // 保存は部品を原本のまま持ち越すので、ここを書き戻しには使わない
    let mut notes: Vec<kumihan::Footnote> = Vec::new();
    let mut note_ids: Vec<(String, bool)> = Vec::new();
    for (part, endnote) in [("word/footnotes.xml", false), ("word/endnotes.xml", true)] {
        let mut nxml = String::new();
        if let Ok(mut f) = zip.by_name(part) {
            if f.read_to_string(&mut nxml).is_ok() && !nxml.is_empty() {
                let (ns, taken) = parse_notes(&nxml, endnote, &media);
                notes.extend(ns);
                note_ids.extend(taken.into_iter().map(|id| (id, endnote)));
            }
        }
    }
    // コメント(comments.xml)。id → 本文。本文の参照より先に読む
    let mut cxml = String::new();
    if let Ok(mut f) = zip.by_name("word/comments.xml") {
        let _ = f.read_to_string(&mut cxml);
    }
    let cmap = parse_comments(&cxml);
    let (mut doc, mut rep) = parse_document_rels(&xml, &media, &cmap, &targets);
    // このアプリのペン(joink)は原文控えから筆へ読み戻す
    extract_ink(&mut doc);
    if !styxml.is_empty() {
        doc.styles = parse_styles(&styxml);
    }
    if !pxml.is_empty() {
        let field = |tag: &str| -> String {
            let open = format!("<{tag}");
            let close = format!("</{tag}>");
            let Some(i) = pxml.find(&open) else { return String::new() };
            let Some(g) = pxml[i..].find('>') else { return String::new() };
            if pxml[i..i + g].ends_with('/') {
                return String::new(); // <tag/> = 空
            }
            let s0 = i + g + 1;
            let Some(e) = pxml[s0..].find(&close) else { return String::new() };
            unesc(&pxml[s0..s0 + e])
        };
        doc.props = kumihan::CoreProps {
            creator: field("dc:creator"),
            title: field("dc:title"),
            keywords: field("cp:keywords"),
            subject: field("dc:subject"),
            description: field("dc:description"),
        };
    }
    doc.footnotes = notes;
    doc.note_ids_taken = note_ids;
    // 注の番号の書式(settings.xml)。**docx の既定はここが知っている** —
    // 黙っていれば脚注は算用数字、**文末脚注はローマ数字の小文字**
    // (Word も LibreOffice もそうする。模型側の既定は算用数字なので、
    //  文末脚注ぶんはここで入れ直す)
    doc.footnote_fmt = note_num_fmt(&sxml, "footnotePr")
        .unwrap_or(kumihan::NoteNumFmt::Decimal);
    doc.endnote_fmt = note_num_fmt(&sxml, "endnotePr")
        .unwrap_or(kumihan::NoteNumFmt::LowerRoman);
    if let Some(i) = sxml.find("<w:autoHyphenation") {
        let head = &sxml[i..(i + 60).min(sxml.len())];
        doc.hyphenate = !(head.contains("w:val=\"0\"") || head.contains("w:val=\"false\""));
    }
    // 縦書き(sectPr の textDirection=tbRl)
    if doc.sect_raw.as_deref().is_some_and(|t| {
        t.contains("textDirection") && t.contains("tbRl")
    }) {
        doc.vertical = true;
    }
    // 文書の保護(readOnly 等)。enforcement が切られていれば保護ではない
    if let Some(i) = sxml.find("<w:documentProtection") {
        let head = &sxml[i..(i + 200).min(sxml.len())];
        let off = head.contains("w:enforcement=\"0\"")
            || head.contains("w:enforcement=\"false\"");
        if !off {
            if let Some(j) = head.find("w:edit=\"") {
                let s2 = j + 8;
                if let Some(e) = head[s2..].find('"') {
                    doc.protection = Some(head[s2..s2 + e].to_string());
                }
            }
        }
    }
    // ヘッダー・フッター(全ページ同じもの = type="default")を部品から読む。
    // 表など、まだ持てないものが入っていたら**編集の対象にしない** —
    // paragraphs を空のまま残せば、保存で原文の部品がそのまま生きる
    if let Some(sect) = doc.sect_raw.clone() {
        for (tag, footer) in [("headerReference", false), ("footerReference", true)] {
            let Some(rid) = hf_ref(&sect, tag) else { continue };
            let Some(target) = targets.get(&rid) else { continue };
            let part = format!("word/{}", target.trim_start_matches('/').trim_start_matches("word/"));
            let mut hxml = String::new();
            match zip.by_name(&part) {
                Ok(mut f) => { let _ = f.read_to_string(&mut hxml); }
                Err(_) => continue,
            }
            let (hdoc, hrep) = parse_document_with(&hxml, &media);
            let which = if footer { "フッター" } else { "ヘッダー" };
            let hf = if footer { &mut doc.footer } else { &mut doc.header };
            hf.part = Some(part);
            if hdoc.tables().next().is_some() {
                rep.note(&format!("{which}の表(編集できないが保存では残る)"));
            } else {
                hf.paragraphs = hdoc.paragraphs().cloned().collect();
                if hf.paragraphs.is_empty() {
                    // 段落の無い部品でも、空の段落を1つ置けば編集できる
                    hf.paragraphs.push(Paragraph::default());
                }
            }
            for (n, k) in hrep.unsupported {
                for _ in 0..k {
                    rep.note(&format!("{which}: {n}"));
                }
            }
        }
        // 透かし(ヘッダーの中の VML)。原文控えからモデルへ引き上げる
        // (保存はモデルから作り直すので、控えは外す — 二重になるため)
        for p in &mut doc.header.paragraphs {
            let mut i = 0;
            while i < p.anchors.len() {
                let a = &p.anchors[i];
                if a.contains("v:textpath") {
                    if let Some(j) = a.find("string=\"") {
                        let s0 = j + 8;
                        if let Some(e) = a[s0..].find('"') {
                            let raw = &a[s0..s0 + e];
                            doc.watermark = Some(raw
                                .replace("&quot;", "\"")
                                .replace("&lt;", "<")
                                .replace("&gt;", ">")
                                .replace("&amp;", "&"));
                            p.anchors.remove(i);
                            continue;
                        }
                    }
                }
                i += 1;
            }
        }
    }
    // 文書の既定書体(styles.xml の docDefaults)。読まないと
    // 明朝の書類がこちらの既定(ゴシック)で表示される
    let mut styles = String::new();
    if let Ok(mut f) = zip.by_name("word/styles.xml") {
        let _ = f.read_to_string(&mut styles);
    }
    if let Some(i) = styles.find("docDefaults") {
        let head = &styles[i..(i + 600).min(styles.len())];
        for key in ["w:eastAsia=\"", "w:ascii=\""] {
            if let Some(j) = head.find(key) {
                let s = j + key.len();
                if let Some(e) = head[s..].find('"') {
                    doc.font = Some(head[s..s + e].to_string());
                    break;
                }
            }
        }
        // 既定の大きさも同じ場所(w:sz)。読まないと、無指定の run を
        // 画面・紙に写すときの基準が文書の言い分とずれる
        if let Some(j) = head.find("<w:sz ") {
            if let Some(k) = head[j..].find("w:val=\"") {
                let s = j + k + 7;
                if let Some(e) = head[s..].find('"') {
                    if let Ok(h) = head[s..s + e].parse::<f32>() {
                        doc.size_pt = Some(h / 2.0);
                    }
                }
            }
        }
    }
    // Word の編集で細切れになった同書式の run を読みで繋ぐ
    // (編集で際限なく増やさない・雛形の {{ }} の保険)
    doc.heal_runs();
    Ok((doc, rep))
}

/// 組み立て中の表。表は入れ子になりうるのでスタックで持つ。
#[derive(Default)]
pub(super) struct TblBuild {
    rows: Vec<Vec<Cellbox>>,
    row: Vec<Cellbox>,
    cell: Vec<Paragraph>,
    /// 列幅(mm)。w:gridCol から
    col_mm: Vec<f32>,
    /// 表のスタイルの名前(w:tblStyle)。定義は持たない — 名前を運ぶだけ
    style: Option<String>,
    /// 表の置き方(tblPr の w:jc)
    align: Option<Align>,
    /// 列幅の固定(w:tblLayout type="fixed")
    fixed_layout: bool,
}

/// twip → mm(1twip = 1/20pt)
pub(super) fn twip_mm(v: f32) -> f32 {
    v * 25.4 / (20.0 * 72.0)
}

/// 原文が使う接頭辞の宣言を ` xmlns:…="…"` の並びとして作る。
/// 解決できない接頭辞があれば None(壊れた XML を書かないため)。
///
/// `skip_self` を立てると、**原文の根の要素が自分で宣言している接頭辞を
/// 出さない**。LibreOffice の書き出す `<m:oMath xmlns:m="…">` がこれで、
/// 重ねて付けると属性が二重になって XML が壊れる(Word は開けない)
pub(super) fn ns_attrs(
    raw: &str,
    decls: &std::collections::BTreeMap<String, String>,
    skip_self: bool,
) -> Option<String> {
    // 既定で分かっているもの(原本に宣言が無くても標準の URI)
    const KNOWN: &[(&str, &str)] = &[
        ("w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"),
        ("m", "http://schemas.openxmlformats.org/officeDocument/2006/math"),
        ("r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
        ("wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
        ("a", "http://schemas.openxmlformats.org/drawingml/2006/main"),
        ("pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"),
        ("v", "urn:schemas-microsoft-com:vml"),
        ("o", "urn:schemas-microsoft-com:office:office"),
        ("w10", "urn:schemas-microsoft-com:office:word"),
        ("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
        ("wp14", "http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing"),
        ("w14", "http://schemas.microsoft.com/office/word/2010/wordml"),
        ("w15", "http://schemas.microsoft.com/office/word/2012/wordml"),
        ("wps", "http://schemas.microsoft.com/office/word/2010/wordprocessingShape"),
        ("wpg", "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"),
        ("a14", "http://schemas.microsoft.com/office/drawing/2010/main"),
    ];
    // 原文に出てくる接頭辞を拾う(要素名と属性名)
    let mut prefixes: std::collections::BTreeSet<String> = Default::default();
    let bytes = raw.as_bytes();
    let is_name = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
    let mut i = 0;
    while i < bytes.len() {
        let at_elem = bytes[i] == b'<';
        let at_attr = bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n';
        if at_elem || at_attr {
            let mut j = i + 1;
            if at_elem && j < bytes.len() && bytes[j] == b'/' {
                j += 1;
            }
            let s = j;
            while j < bytes.len() && is_name(bytes[j]) {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' && j > s {
                // 属性なら後ろに = が要る(値の中の URL を接頭辞と間違えない)
                let mut k = j + 1;
                while k < bytes.len() && is_name(bytes[k]) {
                    k += 1;
                }
                let ok = at_elem || (k < bytes.len() && bytes[k] == b'=');
                if ok {
                    prefixes.insert(raw[s..j].to_string());
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    prefixes.remove("xmlns");
    // 原文の根の要素が自分で宣言している分は出さない(二重宣言を作らない)
    let self_decls: std::collections::BTreeSet<String> = if skip_self {
        let head = raw.find('>').map(|e| &raw[..e]).unwrap_or(raw);
        head.match_indices("xmlns:")
            .map(|(at, _)| {
                let s = at + "xmlns:".len();
                let n: usize = head[s..].bytes().take_while(|c| is_name(*c)).count();
                head[s..s + n].to_string()
            })
            .collect()
    } else {
        Default::default()
    };
    let mut out = String::new();
    for p in &prefixes {
        // `xml:` は XML の定めで**最初から結びついている**。宣言してはいけないし、
        // 解決できない接頭辞として数えてもいけない。
        // LibreOffice の数式は `<m:t xml:space="preserve">` を書くので、
        // ここで弾いていた頃は数式が丸ごと落ちていた(2026-08-10、実物で判明)
        if p == "w" || p == "xml" || self_decls.contains(p) {
            continue; // root で宣言済み / XML の既定 / 原文が自分で宣言している
        }
        let uri = decls
            .get(p)
            .map(|s| s.as_str())
            .or_else(|| KNOWN.iter().find(|(k, _)| k == p).map(|(_, v)| *v))?;
        out.push_str(&format!(" xmlns:{p}=\"{uri}\""));
    }
    Some(out)
}

/// 原文が使う接頭辞(`wp14:` など)の宣言を付けた `<w:r>` に包む。
/// 解決できない接頭辞があれば None(壊れた XML を書かないため)。
pub(super) fn wrap_with_ns(raw: &str, decls: &std::collections::BTreeMap<String, String>) -> Option<String> {
    // 包む `<w:r>` は新しく建てる殻なので、原文の自前宣言は内側に残る。
    // よって skip_self は立てない(内と外で同じ接頭辞を宣言しても壊れない)
    let attrs = ns_attrs(raw, decls, false)?;
    Some(format!("<w:r{attrs}>{raw}</w:r>"))
}

/// 数式(OMML)の原文を、段落の控え(anchors)に置ける形にする。
///
/// **`<w:r>` に包んではいけない。** `m:oMath` と `m:oMathPara` は
/// 型の定めの上で run の中身ではなく、`w:p` の直下に run と**並ぶ**物なので、
/// run に入れると Word が開けない XML になる。だから包まずに、
/// 足りない接頭辞の宣言を**元の開き札そのものへ**差し込む。
pub(super) fn carry_math(raw: &str, decls: &std::collections::BTreeMap<String, String>) -> Option<String> {
    let attrs = ns_attrs(raw, decls, true)?;
    if attrs.is_empty() {
        return Some(raw.to_string());
    }
    // 開き札の名前の直後(`<m:oMath` の後ろ)に差し込む
    let at = raw.find([' ', '>', '/'])?;
    Some(format!("{}{}{}", &raw[..at], attrs, &raw[at..]))
}

/// 数式の画像の代替テキストに付ける印。**この印で始まるものだけ**を原文と
/// 見なす — 人が書いた説明文を式と読み違えない。読み書きの両側で使う
pub(super) const TEX_SIRUSI: &str = "officework:tex:";

/// 原文から表示用の画像を引く。EMU(914400/inch)→ mm は ÷36000。
pub(super) fn image_of(
    raw: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
) -> Option<kumihan::InlineImage> {
    let grab = |pat: &str| -> Option<String> {
        let i = raw.find(pat)? + pat.len();
        let end = raw[i..].find('"')? + i;
        Some(raw[i..end].to_string())
    };
    let rid = grab("r:embed=\"")?;
    let bytes = media.get(&rid)?.clone();
    // wp:extent cx/cy(EMU)。無ければ表示しない(大きさを勝手に決めない)
    let cx: f32 = grab("cx=\"")?.parse().ok()?;
    let cy: f32 = grab("cy=\"")?.parse().ok()?;
    // **数式なら原文(LaTeX)が代替テキストに積んである。** 拾わないと、
    // こちらで書いた数式を開き直したとき絵のままで直せない
    let tex = grab("descr=\"")
        .filter(|d| d.starts_with(TEX_SIRUSI))
        .map(|d| unesc(&d[TEX_SIRUSI.len()..]));
    Some(kumihan::InlineImage { bytes, w_mm: cx / 36000.0, h_mm: cy / 36000.0, tex })
}

/// sectPr から用紙の寸法を読む(twip → mm)。
pub(super) fn parse_sect(raw: &str) -> kumihan::PageSetup {
    let g = |el: &str, at: &str| -> Option<f32> {
        let i = raw.find(el)?;
        let head = &raw[i..(i + 200).min(raw.len())];
        let k = format!("{at}=\"");
        let s = head.find(&k)? + k.len();
        let e = head[s..].find('"')? + s;
        head[s..e].parse::<f32>().ok().map(twip_mm)
    };
    let d = kumihan::PageSetup::default();
    // 段組み(w:cols w:num)。twip ではなく数なので g は使わない
    let cols = raw
        .find("<w:cols")
        .and_then(|i| {
            let head = &raw[i..(i + 200).min(raw.len())];
            let k = "w:num=\"";
            let s = head.find(k)? + k.len();
            let e = head[s..].find('"')? + s;
            head[s..e].parse::<u8>().ok()
        })
        .unwrap_or(1);
    kumihan::PageSetup {
        w_mm: g("<w:pgSz", "w:w").unwrap_or(d.w_mm),
        h_mm: g("<w:pgSz", "w:h").unwrap_or(d.h_mm),
        left_mm: g("<w:pgMar", "w:left").unwrap_or(d.left_mm),
        right_mm: g("<w:pgMar", "w:right").unwrap_or(d.right_mm),
        top_mm: g("<w:pgMar", "w:top").unwrap_or(d.top_mm),
        bottom_mm: g("<w:pgMar", "w:bottom").unwrap_or(d.bottom_mm),
        columns: cols.clamp(1, 8),
    }
}

/// `settings.xml` の `w:footnotePr` / `w:endnotePr` から番号の書式を引く。
/// **その札の中の** `w:numFmt` だけを見る — 文書には他にも `w:numFmt` が
/// あるので、札の範囲を切らずに探すと隣の設定を拾う
fn note_num_fmt(sxml: &str, tag: &str) -> Option<kumihan::NoteNumFmt> {
    let open = format!("<w:{tag}>");
    let close = format!("</w:{tag}>");
    let s0 = sxml.find(&open)? + open.len();
    let e0 = sxml[s0..].find(&close)? + s0;
    let seg = &sxml[s0..e0];
    let k = "<w:numFmt w:val=\"";
    let v0 = seg.find(k)? + k.len();
    let v1 = seg[v0..].find('"')? + v0;
    Some(kumihan::NoteNumFmt::from_docx(&seg[v0..v1]))
}

/// `word/footnotes.xml`(`endnotes.xml`)から脚注の中身を読む。
///
/// 根は `w:footnotes`、その下に `w:footnote` が並ぶ。**`w:type` の付いたものは
/// 本物の脚注ではない**(仕切り線 `separator` と、続きの仕切り
/// `continuationSeparator`)ので飛ばす — 数に入れると番号が2つずれる。
///
/// 中の段落は本文とまったく同じ形なので、**本文の読み手を借りる** —
/// 一つぶんを `w:body` に包み直して通す(根の名前空間の宣言も持ち越す)。
pub(super) fn parse_notes(
    xml: &str,
    endnote: bool,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
) -> (Vec<kumihan::Footnote>, Vec<String>) {
    let mut out = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    // 根の開き札(名前空間の宣言つき)。包み直すときにそのまま使う
    let Some(head_end) = xml.find('>') else { return (out, taken) };
    let root = &xml[..head_end + 1];
    let decls = root
        .find(char::is_whitespace)
        .map(|i| &root[i..root.len() - 1])
        .unwrap_or("")
        .trim_end_matches('/');

    let mut r = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        let start_pos = r.buffer_position() as usize;
        match r.read_event_into(&mut buf) {
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"footnote"
                || local(e.name().as_ref()) == b"endnote" =>
            {
                let id = attr(&e, "id").unwrap_or_default();
                // 仕切り線の定義は脚注ではない
                let kind = attr(&e, "type").unwrap_or_default();
                // **id は種類に関わらず取られている。** 仕切りの id を
                // 控えておかないと、新しい注に同じ番号を選んでしまう
                if !id.is_empty() {
                    taken.push(id.clone());
                }
                let name = e.name().to_owned();
                if r.read_to_end_into(name, &mut Vec::new()).is_err() {
                    break;
                }
                if !kind.is_empty() || id.is_empty() {
                    continue;
                }
                let end = r.buffer_position() as usize;
                let raw = &xml[start_pos..end];
                // 中身だけ取り出して w:body に包み直す
                let Some(inner0) = raw.find('>') else { continue };
                let Some(inner1) = raw.rfind("</") else { continue };
                let inner = &raw[inner0 + 1..inner1];
                let wrapped = format!(
                    "<w:document{decls}><w:body>{inner}</w:body></w:document>");
                let (d, _) = parse_document_with(&wrapped, media);
                out.push(kumihan::Footnote {
                    id,
                    endnote,
                    paragraphs: d.paragraphs().cloned().collect(),
                    added: false,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    (out, taken)
}

/// 脚注・文末脚注の印を run として置く。
///
/// **印の run は字を持たない**(`<w:r><w:footnoteReference w:id="2"/></w:r>`)。
/// 読み手は `</w:t>` で run を積むので、ここで積まないと印は消える。
/// 脚注の**文章**は `word/footnotes.xml` にあり、そこは保存で原本のまま
/// 持ち越される — **落ちていたのは本文の印だけ**で、印が消えると
/// 文章は残ったまま指す物を失う(開くと脚注が消えて見える)。
pub(super) fn note_mark(
    e: &BytesStart,
    n: &[u8],
    para: &mut Option<Vec<Run>>,
    size_pt: Option<f32>,
    font: &Option<String>,
    fmt: &CharFormat,
    rep: &mut Report,
) {
    let endnote = n == b"endnoteReference";
    let Some(id) = attr(e, "id") else {
        // id の無い印は指す先が引けない。作り話をせず、落として報告する
        rep.note("脚注・文末脚注の印(id が無く、保存で失われる)");
        return;
    };
    match para.as_mut() {
        Some(p) => {
            let mut f = fmt.clone();
            f.footnote = Some(kumihan::FootnoteRef { id, endnote });
            p.push(Run { text: String::new(), size_pt, font: font.clone(), fmt: f });
            rep.note("脚注・文末脚注の印(本文には出ないが、保存で残る)");
        }
        // 段落の外に印は置けない(置き場が無い)
        None => rep.note("脚注・文末脚注の印(段落の外。保存で失われる)"),
    }
}

/// 節の種類(`<w:type w:val="continuous"/>`)。無ければ docx の既定 `nextPage`。
pub(super) fn sect_type(raw: &str) -> String {
    let Some(i) = raw.find("<w:type") else { return "nextPage".into() };
    let head = &raw[i..(i + 120).min(raw.len())];
    let k = "w:val=\"";
    let Some(s0) = head.find(k) else { return "nextPage".into() };
    let s0 = s0 + k.len();
    match head[s0..].find('"') {
        Some(e) => head[s0..s0 + e].to_string(),
        None => "nextPage".into(),
    }
}

/// sectPr の中から、全ページ同じヘッダー(フッター)の参照 r:id を引く。
/// `<w:headerReference w:type="default" r:id="rId8"/>`。type 無しは default 扱い。
pub(super) fn hf_ref(sect: &str, tag: &str) -> Option<String> {
    let needle = format!("<w:{tag}");
    let mut at = 0usize;
    while let Some(i) = sect[at..].find(&needle) {
        let s = at + i;
        let e = sect[s..].find('>')? + s;
        let head = &sect[s..e];
        if !head.contains("w:type=") || head.contains("w:type=\"default\"") {
            if let Some(j) = head.find("r:id=\"") {
                let js = j + 6;
                if let Some(je) = head[js..].find('"') {
                    return Some(head[js..js + je].to_string());
                }
            }
        }
        at = e;
    }
    None
}

/// フィールドの命令を印(1字)へ。PAGE(いまのページの番号)と
/// NUMPAGES(総頁)だけを持つ。それ以外は None(報告して落とす)。
pub(super) fn field_mark(instr: &str) -> Option<char> {
    match instr.split_whitespace().next() {
        Some("PAGE") => Some(PAGE_MARK),
        Some("NUMPAGES") => Some(PAGES_MARK),
        _ => None,
    }
}

/// 相互参照の命令(REF / PAGEREF しおり名)。
pub(super) fn ref_instr(instr: &str) -> Option<RefField> {
    let mut it = instr.split_whitespace();
    let kind = it.next()?;
    let name = it.next()?.to_string();
    match kind {
        "REF" => Some(RefField { name, page: false }),
        "PAGEREF" => Some(RefField { name, page: true }),
        _ => None,
    }
}

/// 部分木の原文から、w:t の中の文字を繋いで返す(フィールドの見えている値)。
pub(super) fn inner_texts(raw: &str) -> String {
    let mut out = String::new();
    let mut at = 0usize;
    while let Some(i) = raw[at..].find("<w:t") {
        let s = at + i;
        let Some(gt) = raw[s..].find('>') else { break };
        // <w:tab/> 等に引っ掛からない(<w:t か <w:t␣ だけ)
        let head = &raw[s..s + gt];
        if !(head == "<w:t" || head.starts_with("<w:t ")) {
            at = s + 4;
            continue;
        }
        let ts = s + gt + 1;
        let Some(te) = raw[ts..].find("</w:t>") else { break };
        out.push_str(
            &raw[ts..ts + te]
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&amp;", "&"),
        );
        at = ts + te;
    }
    out
}

/// comments.xml を読む。id → コメント(書いた人・本文)。
pub(super) fn parse_comments(xml: &str) -> std::collections::BTreeMap<String, Comment> {
    let mut out: std::collections::BTreeMap<String, Comment> = Default::default();
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut cur: Option<(String, Comment)> = None;
    let mut in_t = false;
    loop {
        match r.read_event_into(&mut buf) {
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"comment" => {
                    let id = attr(&e, "id").unwrap_or_default();
                    let author = attr(&e, "author").unwrap_or_default();
                    cur = Some((id, Comment { author, text: String::new() }));
                }
                b"p" => {
                    if let Some((_, c)) = cur.as_mut() {
                        if !c.text.is_empty() {
                            c.text.push('\n');
                        }
                    }
                }
                b"t" => in_t = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => {
                if let Some((_, c)) = cur.as_mut() {
                    c.text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"t" => in_t = false,
                b"comment" => {
                    if let Some((id, c)) = cur.take() {
                        out.insert(id, c);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    out
}

/// XML の属性・本文に入れる文字を逃がす。
/// esc の逆(core.xml など小さな部品の中身を戻す)
pub(super) fn unesc(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

pub(super) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// comments.xml を作る。id は 1 から(document.xml 側の参照と同じ振り方)。
pub(super) fn comments_xml(cmts: &[Comment]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<w:comments xmlns:w=\"{W_NS}\">"));
    for (i, c) in cmts.iter().enumerate() {
        out.push_str(&format!(
            r#"<w:comment w:id="{}" w:author="{}">"#, i + 1, esc(&c.author)));
        for line in c.text.split('\n') {
            out.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            out.push_str(&esc(line));
            out.push_str("</w:t></w:r></w:p>");
        }
        out.push_str("</w:comment>");
    }
    out.push_str("</w:comments>");
    out
}

/// 透かしの図形(VML)。Word が透かしに使う形(WordArt t136)に合わせる。
/// shapetype(字形の定義)ごと1つの w:pict に入れる。
pub(super) fn watermark_vml(text: &str) -> String {
    format!(
        concat!(
            r#"<w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e">"#,
            r#"<v:formulas><v:f eqn="sum #0 0 10800"/><v:f eqn="prod #0 2 1"/><v:f eqn="sum 21600 0 @1"/><v:f eqn="sum 0 0 @2"/><v:f eqn="sum 21600 0 @3"/><v:f eqn="if @0 @3 0"/><v:f eqn="if @0 21600 @1"/><v:f eqn="if @0 0 @2"/><v:f eqn="if @0 @4 21600"/><v:f eqn="mid @5 @6"/><v:f eqn="mid @8 @5"/><v:f eqn="mid @7 @8"/><v:f eqn="mid @6 @7"/><v:f eqn="sum @6 0 @5"/></v:formulas>"#,
            r#"<v:path textpathok="t" o:connecttype="custom" o:connectlocs="@9,0;@10,10800;@11,21600;@12,10800" o:connectangles="270,180,90,0"/>"#,
            r#"<v:textpath on="t" fitshape="t"/>"#,
            r##"<v:handles><v:h position="#0,bottommost" xrange="6629,14971"/></v:handles>"##,
            r#"<o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype>"#,
            r##"<v:shape id="jowatermark" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:460pt;height:230pt;rotation:315;z-index:-251654144;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="#d8d8d8" stroked="f">"##,
            r#"<v:fill opacity=".5"/><v:textpath style="font-family:&quot;游明朝&quot;" string="{}"/></v:shape></w:pict>"#
        ),
        esc(text)
    )
}

/// mm → EMU(914400/インチ)
pub(super) fn emu(mm: f32) -> i64 {
    (mm * 36000.0).round() as i64
}

/// 手描きの1筆を、ページ固定の自由曲線(DrawingML)にする。
/// 名前 `joink…p{page}` が読み戻しの鍵(ページ番号も名前で持つ —
/// XML の座標はページの中の位置しか持てないため)。
/// アプリ(writer)が、そのページにある段落の anchors に差し込む。
pub fn ink_anchor_xml(st: &Stroke, id: usize) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in &st.points {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    let (w, h) = ((x1 - x0).max(0.5), (y1 - y0).max(0.5));
    let mut path = String::new();
    for (i, (x, y)) in st.points.iter().enumerate() {
        let tag = if i == 0 { "moveTo" } else { "lnTo" };
        path.push_str(&format!(
            r#"<a:{tag}><a:pt x="{}" y="{}"/></a:{tag}>"#,
            emu(x - x0),
            emu(y - y0)
        ));
    }
    // 蛍光ペンは太く・薄く・文字の下(behindDoc)。ペンは細く・濃く・上
    let (line_w, color, alpha, behind) = if st.highlighter {
        (emu(3.0), "FFE45C", r#"<a:alpha val="45000"/>"#, "1")
    } else {
        (emu(0.45), "1B3A52", "", "0")
    };
    format!(
        concat!(
            r#"<w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" "#,
            r#"relativeHeight="251658240" behindDoc="{behind}" locked="0" layoutInCell="1" allowOverlap="1">"#,
            r#"<wp:simplePos x="0" y="0"/>"#,
            r#"<wp:positionH relativeFrom="page"><wp:posOffset>{px}</wp:posOffset></wp:positionH>"#,
            r#"<wp:positionV relativeFrom="page"><wp:posOffset>{py}</wp:posOffset></wp:positionV>"#,
            r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:wrapNone/>"#,
            r#"<wp:docPr id="{id}" name="joink{id}p{page}"/>"#,
            r#"<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"<wps:wsp><wps:cNvSpPr/><wps:spPr>"#,
            r#"<a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>"#,
            r#"<a:rect l="0" t="0" r="{cx}" b="{cy}"/>"#,
            r#"<a:pathLst><a:path w="{cx}" h="{cy}">{path}</a:path></a:pathLst></a:custGeom>"#,
            r#"<a:noFill/>"#,
            r#"<a:ln w="{lw}" cap="rnd"><a:solidFill><a:srgbClr val="{color}">{alpha}</a:srgbClr></a:solidFill><a:round/></a:ln>"#,
            r#"</wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing>"#
        ),
        behind = behind,
        px = emu(x0),
        py = emu(y0),
        cx = emu(w),
        cy = emu(h),
        id = id,
        page = st.page,
        path = path,
        lw = line_w,
        color = color,
        alpha = alpha,
    )
}

/// [`ink_anchor_xml`] を、段落の控え(anchors)にそのまま置ける形
/// (名前空間の宣言つきの `<w:r>`)で返す。writer はこれを差し込む。
pub fn ink_anchor_run(st: &Stroke, id: usize) -> String {
    let inner = ink_anchor_xml(st, id);
    wrap_with_ns(&inner, &Default::default()).unwrap_or(inner)
}

/// 原文控え(anchors)の中の joink(手描きの線)を筆に読み戻す。
/// 読めたら控えから外す(保存はモデルから作り直す)。
pub(super) fn extract_ink(doc: &mut Document) {
    let grab = |s: &str, pat: &str| -> Option<f32> {
        let i = s.find(pat)? + pat.len();
        let e = s[i..].find('<')? + i;
        s[i..e].parse::<f32>().ok()
    };
    let mut ink: Vec<Stroke> = Vec::new();
    for b in &mut doc.blocks {
        let Block::Para(p) = b else { continue };
        let mut i = 0;
        while i < p.anchors.len() {
            let a = &p.anchors[i];
            let Some(ni) = a.find("name=\"joink") else {
                i += 1;
                continue;
            };
            // 名前からページ番号(joink{id}p{page})
            let page = a[ni..]
                .find('p')
                .and_then(|pi| {
                    let s2 = &a[ni + pi + 1..];
                    let e = s2.find('"')?;
                    s2[..e].parse::<usize>().ok()
                })
                .unwrap_or(0);
            let x0 = grab(a, "<wp:positionH relativeFrom=\"page\"><wp:posOffset>")
                .map(|v| v / 36000.0);
            let y0 = grab(a, "<wp:positionV relativeFrom=\"page\"><wp:posOffset>")
                .map(|v| v / 36000.0);
            let (Some(x0), Some(y0)) = (x0, y0) else {
                i += 1;
                continue;
            };
            // a:pt を順に拾う
            let mut points: Vec<(f32, f32)> = Vec::new();
            let mut at = 0usize;
            while let Some(j) = a[at..].find("<a:pt x=\"") {
                let s2 = at + j + 9;
                let Some(xe) = a[s2..].find('"') else { break };
                let Some(yj) = a[s2 + xe..].find("y=\"") else { break };
                let ys = s2 + xe + yj + 3;
                let Some(ye) = a[ys..].find('"') else { break };
                if let (Ok(x), Ok(y)) =
                    (a[s2..s2 + xe].parse::<f32>(), a[ys..ys + ye].parse::<f32>())
                {
                    points.push((x0 + x / 36000.0, y0 + y / 36000.0));
                }
                at = ys + ye;
            }
            if points.is_empty() {
                i += 1;
                continue;
            }
            let highlighter = a.contains("<a:alpha");
            ink.push(Stroke { page, highlighter, points });
            p.anchors.remove(i);
        }
    }
    doc.ink.extend(ink);
}

/// styles.xml から スタイルの名乗り(id・名前・種類)を写す。
/// 浅い読み(core.xml と同じ流儀)— 定義の本体は理解せず、原本が持ち越す。
pub(super) fn parse_styles(xml: &str) -> Vec<kumihan::StyleInfo> {
    fn attr_of(hay: &str, key: &str) -> String {
        let pat = format!("{key}=\"");
        hay.find(&pat)
            .and_then(|s| {
                let s = s + pat.len();
                hay[s..].find('"').map(|e| unesc(&hay[s..s + e]))
            })
            .unwrap_or_default()
    }
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = xml[from..].find("<w:style ").map(|i| i + from) {
        let head_end = match xml[i..].find('>') {
            Some(e) => i + e,
            None => break,
        };
        let head = &xml[i..head_end];
        // <w:style …/> と <w:style …>…</w:style> の両方を受ける
        let end = if head.ends_with('/') {
            head_end
        } else {
            xml[head_end..].find("</w:style>").map(|e| head_end + e).unwrap_or(xml.len())
        };
        let id = attr_of(head, "w:styleId");
        let kind = attr_of(head, "w:type");
        let body = &xml[head_end..end];
        let name = body
            .find("<w:name ")
            .map(|n| {
                let seg = &body[n..body[n..].find('>').map(|e| n + e).unwrap_or(body.len())];
                attr_of(seg, "w:val")
            })
            .unwrap_or_default();
        if !id.is_empty() {
            out.push(kumihan::StyleInfo { id, name, kind });
        }
        from = end.max(i + 1);
    }
    out
}

/// `w:pStyle` の val を段落の役割へ。見出しと目次の行だけを見る
/// (それ以外のスタイルは今まで通り持たない)。
/// 見出しの style id は日本語版 Word が「1」、英語版が「Heading1」。
pub(super) fn style_of(val: &str) -> ParaStyle {
    let v = val.to_ascii_lowercase().replace(' ', "");
    match v.as_str() {
        "1" | "heading1" | "見出し1" => ParaStyle::Heading(1),
        "2" | "heading2" | "見出し2" => ParaStyle::Heading(2),
        "3" | "heading3" | "見出し3" => ParaStyle::Heading(3),
        "tableoffigures" => ParaStyle::Tof,
        "toc1" => ParaStyle::Toc(1),
        "toc2" => ParaStyle::Toc(2),
        "toc3" => ParaStyle::Toc(3),
        _ => ParaStyle::Body,
    }
}

/// w:fldChar(複雑なフィールドの区切り)を1つ処理する。
/// begin〜end の間に instrText で命令が来る。separate〜end は
/// 「計算済みの見た目」なので持たない(開く側が計算し直す)。
#[allow(clippy::too_many_arguments)]
pub(super) fn fldchar(
    kind: Option<&str>,
    in_field: &mut bool,
    field_hide: &mut bool,
    field_instr: &mut String,
    field_buf: &mut String,
    para: &mut Option<Vec<Run>>,
    rep: &mut Report,
    size_pt: Option<f32>,
    font: &Option<String>,
    fmt: &CharFormat,
) {
    match kind {
        Some("begin") => {
            *in_field = true;
            *field_hide = false;
            field_instr.clear();
            field_buf.clear();
        }
        Some("separate") => {
            if *in_field {
                *field_hide = true;
            }
        }
        Some("end")
            if *in_field => {
                *in_field = false;
                *field_hide = false;
                if let Some(mark) = field_mark(field_instr) {
                    if let Some(p) = para.as_mut() {
                        p.push(Run {
                            text: mark.to_string(),
                            size_pt,
                            font: font.clone(),
                            fmt: fmt.clone(),
                        });
                    }
                } else if let Some(rf) = ref_instr(field_instr) {
                    // 相互参照。separate〜end の見えている値ごと run にする
                    if let Some(p) = para.as_mut() {
                        let mut f2 = fmt.clone();
                        f2.field = Some(rf);
                        p.push(Run {
                            text: std::mem::take(field_buf),
                            size_pt,
                            font: font.clone(),
                            fmt: f2,
                        });
                    }
                } else if !field_instr.trim().is_empty() {
                    rep.note(&format!("フィールド({})", field_instr.trim()));
                }
            }
        _ => {}
    }
}

pub fn parse_document_xml(xml: &str) -> (Document, Report) {
    parse_document_with(xml, &Default::default())
}

/// media は 関係ID(rId5)→ 画像の実体。`read` が rels と media から作る。
pub fn parse_document_with(
    xml: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
) -> (Document, Report) {
    parse_document_full(xml, media, &Default::default())
}

/// cmts は comments.xml の中身(id → コメント)。参照をここで解決する。
/// w:sdtPr の子要素を読んで記入欄の設定を組み立てる。返り値: 処理したか。
/// **Start でも Empty でも同じ**(docx は空要素で書くのが普通)
pub(super) fn sdt_pr_elem(
    name: &[u8],
    e: &quick_xml::events::BytesStart,
    sd: &mut Option<kumihan::Sdt>,
) -> bool {
    use kumihan::SdtKind as K;
    match name {
        b"alias" => {
            if let Some(v) = attr(e, "val") {
                sd.get_or_insert_with(Default::default).alias = v;
            }
        }
        b"tag" => {
            if let Some(v) = attr(e, "val") {
                let s = sd.get_or_insert_with(Default::default);
                // 「jo:email:連絡先」= うちだけの種類の印+名前(名前ボタン)。
                // 解いてモデルの tag には名前だけを持つ(書く側が組み直す)
                if let Some((k, name)) = K::split_tag(&v) {
                    s.kind = k;
                    s.tag = name;
                } else {
                    s.tag = v;
                }
            }
        }
        b"comboBox" => sd.get_or_insert_with(Default::default).kind = K::Combo,
        b"dropDownList" => sd.get_or_insert_with(Default::default).kind = K::Dropdown,
        b"listItem" => {
            if let Some(v) = attr(e, "value").or_else(|| attr(e, "displayText")) {
                sd.get_or_insert_with(Default::default).items.push(v);
            }
        }
        b"checkbox" => sd.get_or_insert_with(Default::default).kind = K::Checkbox,
        b"picture" => sd.get_or_insert_with(Default::default).kind = K::Picture,
        b"date" => sd.get_or_insert_with(Default::default).kind = K::Date,
        // 素の記入欄。種類が決まっていなければ Text
        b"text" => {
            sd.get_or_insert_with(Default::default);
        }
        _ => return false,
    }
    true
}

pub(super) fn parse_document_full(
    xml: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    cmts: &std::collections::BTreeMap<String, Comment>,
) -> (Document, Report) {
    parse_document_rels(xml, media, cmts, &Default::default())
}

/// 関係(rId → 的)つき。リンクの URL を解くのに要る
pub(super) fn parse_document_rels(
    xml: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    cmts: &std::collections::BTreeMap<String, Comment>,
    targets: &std::collections::BTreeMap<String, String>,
) -> (Document, Report) {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);

    let mut doc = Document::default();
    let mut rep = Report::default();
    let mut stack: Vec<TblBuild> = Vec::new();

    let mut para: Option<Vec<Run>> = None;
    // いま読んでいるセルの結合(w:tcPr の gridSpan / vMerge)
    let mut cell_span = 0u8;
    let mut cell_vmerge = VMerge::None;
    // **無指定は None のまま持つ。** ここで数を入れると、往復で
    // 「10.5pt 指定」が焼き付く(2026-08-13、本家 python-docx で発覚)
    let mut size_pt: Option<f32> = None;
    // **書体は文書の設定**。docx が w:rFonts で持っているものを捨てない
    let mut font: Option<String> = None;
    // 文字の書式(w:rPr)と段落の揃え(w:jc)。読んで捨てると開き直したとき消える
    let mut fmt = CharFormat::default();
    let mut align = Align::default();
    // 箇条書き・インデント・行間(w:numPr / w:ind / w:spacing)
    let mut list = ListKind::default();
    let mut indent = 0u8;
    let mut first_line = 0i32; // w:ind の firstLine(正)/ hanging(負)。twip のまま持つ
    let mut line_spacing = 1.0f32;
    let mut page_break_before = false;
    // 段落の背景色(w:shd)と囲み枠(w:pBdr)
    let mut shade: Option<String> = None;
    let mut boxed = false;
    // 段落の役割(w:pStyle / w:outlineLvl)
    let mut pstyle = ParaStyle::Body;
    let mut pstyle_id: Option<String> = None;
    // リストの深さ(w:ilvl)。w:ind が無い文書ではこれがインデントになる
    let mut ilvl = 0u8;
    // この段落に付いたコメント(commentReference を解決したもの)
    let mut para_comments: Vec<Comment> = Vec::new();
    // この段落に付いたしおり(bookmarkStart の名前)
    let mut para_bookmarks: Vec<String> = Vec::new();
    // ドロップキャップ。Word は「枠の段落(頭の1字)+本文の段落」に割るので、
    // 枠の段落を控えて次の段落の頭に合流させる
    let mut dropcap = false;
    let mut pending_cap: Option<Vec<Run>> = None;
    // 読めなかった要素の原文(画像など)。段落ごとに集めて持ち越す
    let mut anchors: Vec<String> = Vec::new();
    // この段落で終わる節(w:pPr の中の w:sectPr)。段落を閉じるとき渡す
    let mut para_sect: Option<kumihan::SectionBreak> = None;
    // 表示できる画像(r:embed と wp:extent が読めたもの)
    let mut images: Vec<kumihan::InlineImage> = Vec::new();
    // 原本の root が宣言している名前空間。持ち越す原文の接頭辞をこれで包む
    let mut ns_decls: std::collections::BTreeMap<String, String> = Default::default();
    let mut in_ppr = false;
    let mut in_tblpr = false;
    // 記入欄(w:sdt)。sdtPr を読んで控え、sdtContent の中の run に付ける
    let mut sdt_depth = 0usize;
    let mut in_sdtpr = false;
    let mut sdt_now: Option<kumihan::Sdt> = None;
    let mut sdt_cur: Option<Box<kumihan::Sdt>> = None;
    let mut in_text = false;
    let mut in_rpr = false;
    let mut cur_link: Option<String> = None;
    let mut cur = String::new();
    // フィールド(w:fldChar / w:instrText)。PAGE は印、REF は参照の run になる
    let mut in_instr = false;
    let mut in_field = false;
    let mut field_hide = false;
    let mut field_instr = String::new();
    let mut field_buf = String::new();

    let mut buf = Vec::new();
    // 直前のイベントの終わり位置(原文の切り出しに使う)
    let mut last_pos = r.buffer_position() as usize;
    loop {
        let ev = r.read_event_into(&mut buf);
        let start_pos = last_pos;
        last_pos = r.buffer_position() as usize;
        match ev {
            Err(e) => { rep.note(&format!("XML解析エラー: {e}")); break }
            Ok(Event::Eof) => {
                // 枠の段落だけで終わった文書。段落として残す(黙って捨てない)
                if let Some(cap) = pending_cap.take() {
                    doc.push_para(Paragraph {
                        dropcap: true,
                        line_spacing: 1.0,
                        runs: cap,
                        ..Default::default()
                    });
                }
                break;
            }
            Ok(Event::Start(e)) => {
                let n = local(e.name().as_ref()).to_vec();
                match n.as_slice() {
                    // hdr / ftr はヘッダー・フッターの部品を読むときの root
                    b"document" | b"hdr" | b"ftr" => {
                        // root の xmlns:* を控える(画像の原文の接頭辞に要る)
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            if let Some(pfx) = k.strip_prefix("xmlns:") {
                                if let Ok(v) = a.unescape_value() {
                                    ns_decls.insert(pfx.to_string(), v.to_string());
                                }
                            }
                        }
                    }
                    b"tbl" => stack.push(TblBuild::default()),
                    b"gridCol" => if let Some(b) = stack.last_mut() {
                        if let Some(w) = attr(&e, "w").and_then(|v| v.parse::<f32>().ok()) {
                            b.col_mm.push(twip_mm(w));
                        }
                    },
                    b"tr" => if let Some(b) = stack.last_mut() { b.row.clear() },
                    b"tc" => if let Some(b) = stack.last_mut() {
                        b.cell.clear();
                        cell_span = 0;
                        cell_vmerge = VMerge::None;
                    },
                    b"p" => { para = Some(Vec::new()); size_pt = None; font = None;
                              fmt = CharFormat::default(); align = Align::default();
                              list = ListKind::default(); indent = 0; first_line = 0;
                              line_spacing = 1.0;
                              page_break_before = false; shade = None; boxed = false;
                              pstyle = ParaStyle::Body; pstyle_id = None; ilvl = 0;
                              para_comments.clear(); para_bookmarks.clear();
                              dropcap = false; }
                    b"rPr" => {
                        in_rpr = true;
                        fmt = CharFormat { sdt: sdt_cur.clone(), link: cur_link.clone(), ..Default::default() };
                    }
                    b"pPr" => in_ppr = true,
                    b"tblPr" => in_tblpr = true,
                    // リンク。**外部の的(URL)は関係から解く** — 解けない
                    // 内部リンク(しおりへ)は今は持たない(字は残る)
                    b"hyperlink" => {
                        cur_link = attr(&e, "id").and_then(|id| targets.get(&id).cloned());
                    }
                    b"sz" if in_rpr => {
                        if let Some(v) = attr(&e, "val") {
                            if let Ok(h) = v.parse::<f32>() { size_pt = Some(h / 2.0); }
                        }
                    }
                    // 日本語の書体は eastAsia に入る。ascii しか見ないと明朝が消える
                    b"rFonts" if in_rpr => {
                        font = attr(&e, "eastAsia")
                            .or_else(|| attr(&e, "ascii"))
                            .or_else(|| attr(&e, "hAnsi"))
                            .filter(|s| !s.is_empty());
                    }
                    // w:val="0"/"false" は「付けない」の意味なので、有無だけで判定しない
                    b"b" if in_rpr => fmt.bold = on(&e),
                    b"i" if in_rpr => fmt.italic = on(&e),
                    b"u" if in_rpr => fmt.underline = attr(&e, "val").as_deref() != Some("none"),
                    b"strike" if in_rpr => fmt.strike = on(&e),
                    b"color" if in_rpr => {
                        fmt.color = attr(&e, "val").filter(|v| !v.is_empty() && v != "auto");
                    }
                    b"vertAlign" if in_rpr => {
                        match attr(&e, "val").as_deref() {
                            Some("superscript") => fmt.superscript = true,
                            Some("subscript") => fmt.subscript = true,
                            _ => {}
                        }
                    }
                    b"highlight" if in_rpr => {
                        fmt.highlight = attr(&e, "val").filter(|v| v != "none");
                    }

                    // 箇条書きは numId で決まる。1 を中黒、2 を段落番号として扱う
                    // (numbering.xml を持たないので、往復できる最小の約束にしてある)
                    b"ilvl" if in_ppr => {
                        ilvl = attr(&e, "val").and_then(|v| v.parse().ok()).unwrap_or(0).min(8);
                    }
                    b"numId" if in_ppr => {
                        list = match attr(&e, "val").as_deref() {
                            Some("2") => ListKind::Number,
                            Some("0") | None => ListKind::None,
                            _ => ListKind::Bullet,
                        };
                    }
                    // 段落のスタイル。見出しと目次の行だけを持つ
                    b"pStyle" if in_ppr => {
                        if let Some(v) = attr(&e, "val") {
                            pstyle = style_of(&v);
                            // 役割を知らないスタイル名も**捨てない** — 原文のまま運ぶ
                            pstyle_id = Some(v);
                        }
                    }
                    // 文字スタイル。名前を運んで返すだけ(定義は styles.xml)
                    b"rStyle" if in_rpr => {
                        fmt.style_id = attr(&e, "val").filter(|v| !v.is_empty());
                    }
                    // スタイル名で見出しと分からなくても、outlineLvl があれば見出し
                    b"outlineLvl" if in_ppr => {
                        if pstyle == ParaStyle::Body {
                            if let Some(n) = attr(&e, "val").and_then(|v| v.parse::<u8>().ok()) {
                                if n < 3 { pstyle = ParaStyle::Heading(n + 1); }
                            }
                        }
                    }
                    b"framePr" if in_ppr => {
                        dropcap = matches!(attr(&e, "dropCap").as_deref(),
                            Some("drop") | Some("margin"));
                    }
                    b"pageBreakBefore" if in_ppr => {
                        page_break_before = on(&e);
                    }
                    b"ind" if in_ppr => {
                        // twip。1段 = 全角2文字 = 10.5pt×2 ≒ 420twip
                        indent = attr(&e, "left")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 420.0).round().clamp(0.0, 20.0) as u8)
                            .unwrap_or(0);
                        // 1行目の字下げは twip のまま(段落を触っても落とさない —
                        // 2026-08-13 に「黙って消える」を実測で踏んだ)
                        first_line = attr(&e, "firstLine")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v as i32)
                            .or_else(|| {
                                attr(&e, "hanging")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -(v as i32))
                            })
                            .unwrap_or(0);
                    }
                    b"spacing" if in_ppr => {
                        // w:line は 240 = 1行
                        line_spacing = attr(&e, "line")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 240.0).clamp(0.5, 5.0))
                            .unwrap_or(1.0);
                    }
                    b"jc" if in_ppr => {
                        if let Some(v) = attr(&e, "val") { align = Align::from_docx(&v); }
                    }
                    // 表の置き方・スタイル名・列幅の固定(w:tblPr の中)
                    b"jc" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.align = attr(&e, "val").map(|v| Align::from_docx(&v));
                    },
                    b"tblStyle" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.style = attr(&e, "val").filter(|v| !v.is_empty());
                    },
                    b"tblLayout" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.fixed_layout = attr(&e, "type").as_deref() == Some("fixed");
                    },
                    // 段落の背景色。fill が色(auto 以外)のときだけ
                    b"shd" if in_ppr => {
                        shade = attr(&e, "fill")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // 段落の囲み枠。辺の別は持たない(あれば囲みとみなす)
                    b"pBdr" if in_ppr => boxed = true,
                    b"r" => {
                        // 大きさは run ごとに立ち返る。前の run の指定を
                        // 引きずると、無指定の run が「指定あり」に化ける
                        // (書体 font には同じ形の持ち回りがまだ残っている —
                        // 直すなら別の回で、試験と一緒に)
                        size_pt = None;
                        fmt.sdt = sdt_cur.clone();
                        // **rPr の無い run にもリンクは掛かる** — 掛かりを
                        // 決めるのは囲み(w:hyperlink)で、run の書式ではない
                        fmt.link = cur_link.clone();
                    }
                    b"t" => { in_text = true; cur.clear(); }
                    // 脚注・文末脚注の印。空要素で来るのが普通なので実際に効くのは
                    // Empty の枝だが、**両方の枝に置く** — 片方の枝でしか見ていない
                    // せいで実物を取りこぼした前科がある(xlsx の sheetView)
                    b"footnoteReference" | b"endnoteReference" =>
                        note_mark(&e, &n, &mut para, size_pt, &font, &fmt, &mut rep),
                    // セル結合。横は列数、縦は restart/continue の区別で持つ
                    b"gridSpan" => if stack.last().is_some() {
                        cell_span = attr(&e, "val").and_then(|v| v.parse().ok()).unwrap_or(0);
                    },
                    b"vMerge" => if stack.last().is_some() {
                        cell_vmerge = match attr(&e, "val").as_deref() {
                            Some("restart") => VMerge::Start,
                            // val 無しは「続き」(docx の既定)
                            _ => VMerge::Continue,
                        };
                    },
                    b"sectPr" => {
                        // 節の設定。用紙・余白のほか、ヘッダーの参照も入っている。
                        // **理解はしないが捨てない**(捨てると保存で用紙設定と
                        // ヘッダーが消える)。寸法だけは読んで組版に使う。
                        //
                        // docx は節を**2か所に書き分ける**:
                        //   * 途中の節 … その節の最後の段落の `w:pPr` の中
                        //   * 最後の節 … `w:body` の直下(段落の外)
                        // 段落の中に居るかどうかがそのまま見分けになる
                        let name = e.name().to_owned();
                        if r.read_to_end_into(name, &mut Vec::new()).is_ok() {
                            let end = r.buffer_position() as usize;
                            let raw = &xml[start_pos..end];
                            if para.is_some() {
                                // 途中の節の区切り。**段落に持たせて保存で返す**
                                // (以前はここで doc.sect_raw を上書きしていて、
                                // 区切りごと保存で消えていた)。
                                // 用紙は最後の節のもので組むので、そこは言う
                                // 原文・組版の顔・改ページするかを**一緒に**持つ
                                para_sect = Some(kumihan::SectionBreak {
                                    raw: raw.to_string(),
                                    page: parse_sect(raw),
                                    continuous: sect_type(raw) == "continuous",
                                });
                                rep.note("節の区切り(用紙ごと組み直す。保存でも残る)");
                            } else {
                                // 最後の節。用紙・ヘッダーの参照はここから読む
                                doc.page = Some(parse_sect(raw));
                                doc.sect_raw = Some(raw.to_string());
                            }
                            last_pos = end;
                        }
                    }
                    // 数式(OMML)。**理解はしないが、捨てない** — sect_raw や
                    // 画像と同じ作法で、原文を丸ごと控えて保存で返す。
                    //
                    // 部分木は読み飛ばす。これが本題でもある: `local()` は接頭辞を
                    // 落とすので `<m:t>` は `b"t"` の枝に落ち、**数式の中の字が
                    // 本文に混ざっていた**(そして保存すると数式ではなく平文に
                    // なっていた)。読み飛ばせば漏れも止まる。
                    //
                    // `oMathPara`(独立した数式)を先に捕まえるので、その中の
                    // `oMath` は read_to_end に呑まれる = 二重に控えない
                    b"oMath" | b"oMathPara" => {
                        let name = e.name().to_owned();
                        if r.read_to_end_into(name, &mut Vec::new()).is_ok() {
                            let end = r.buffer_position() as usize;
                            let raw = &xml[start_pos..end];
                            match carry_math(raw, &ns_decls) {
                                // 段落の並びの中の位置は失われ、段落の頭に寄る
                                // (画像と同じ、正直な限界)。文そのものは残る
                                Some(carried) => {
                                    anchors.push(carried);
                                    rep.note("数式(段落の頭に寄るが、保存で残る)");
                                }
                                None => {
                                    // 出どころの分からない接頭辞。壊れた XML を
                                    // 書くより、落として報告する方がまし
                                    rep.note("数式(接頭辞が解決できず、保存で失われる)");
                                }
                            }
                            last_pos = end;
                        }
                    }
                    b"drawing" | b"pict" | b"object" => {
                        // **理解はしないが、捨てない。** 原文を丸ごと控えて保存で返す。
                        // 部分木は読み飛ばす — 中の a:t(図形の文字)を本文に
                        // 混ぜないため(以前はここから本文へ漏れていた)
                        let name = e.name().to_owned();
                        if r.read_to_end_into(name, &mut Vec::new()).is_ok() {
                            let end = r.buffer_position() as usize;
                            let raw = &xml[start_pos..end];
                            // 原文が使う接頭辞の宣言を、包む run に付ける。
                            // 付けないと保存した XML が「未宣言の接頭辞」で壊れる
                            // 表示用: 画像の実体と大きさが分かれば拾う
                            if let Some(im) = image_of(raw, media) {
                                images.push(im);
                            }
                            match wrap_with_ns(raw, &ns_decls) {
                                Some(wrapped) => {
                                    anchors.push(wrapped);
                                    rep.note("画像・図形(保存では残る)");
                                }
                                None => {
                                    // 出どころの分からない接頭辞。壊れたXMLを書くより
                                    // 落として報告する方がまし
                                    rep.note("画像・図形(接頭辞が解決できず、保存で失われる)");
                                }
                            }
                            last_pos = end;
                        }
                    }
                    // 記入欄(コンテンツコントロール)。殻を読んで、
                    // 中の run に印を付ける(中身は普通の本文として読む)
                    b"sdt" => {
                        sdt_depth += 1;
                    }
                    b"sdtPr" => in_sdtpr = true,
                    _ if in_sdtpr && sdt_pr_elem(&n, &e, &mut sdt_now) => {}
                    b"sdtContent" => {
                        // ここから中身。以後の run に欄の印が付く
                        sdt_cur = sdt_now.take().map(Box::new);
                    }
                    // 既にある変更履歴。挿入(w:ins)の中の字は本文として読み、
                    // 削除(w:del)の中(w:delText)は読まない = 確定後の姿。
                    // **保存すると履歴そのものは消える**ので、そう言う
                    b"ins" | b"del" => {
                        rep.note("変更履歴(表示は確定後の姿。保存で履歴は確定される)");
                    }
                    // ページの色(w:background。root 直下)
                    b"background" => {
                        doc.page_color = attr(&e, "color")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // 単純なフィールド。PAGE は印として持ち、それ以外は報告する
                    b"fldSimple" => {
                        let instr = attr(&e, "instr").unwrap_or_default();
                        let name = e.name().to_owned();
                        if r.read_to_end_into(name, &mut Vec::new()).is_ok() {
                            last_pos = r.buffer_position() as usize;
                        }
                        if let Some(mark) = field_mark(&instr) {
                            if let Some(p) = para.as_mut() {
                                p.push(Run { text: mark.to_string(), size_pt,
                                             font: font.clone(), fmt: fmt.clone() });
                            }
                        } else if let Some(rf) = ref_instr(&instr) {
                            // 相互参照。見えている値ごと run にする
                            let raw = &xml[start_pos..last_pos];
                            if let Some(p) = para.as_mut() {
                                let mut f2 = fmt.clone();
                                f2.field = Some(rf);
                                p.push(Run { text: inner_texts(raw), size_pt,
                                             font: font.clone(), fmt: f2 });
                            }
                        } else {
                            rep.note(&format!("フィールド({})", instr.trim()));
                        }
                    }
                    // ルビ(w:ruby)。読みは w:rt、基底は w:rubyBase から。
                    // 基底の中の書式は落ちる(基底はふつう一様なので許す)
                    b"ruby" => {
                        let name = e.name().to_owned();
                        if r.read_to_end_into(name, &mut Vec::new()).is_ok() {
                            let end = r.buffer_position() as usize;
                            let raw = &xml[start_pos..end];
                            last_pos = end;
                            let slice = |tag: &str| -> &str {
                                let open = format!("<w:{tag}");
                                let close = format!("</w:{tag}>");
                                match (raw.find(&open), raw.find(&close)) {
                                    (Some(a), Some(b)) if b > a => &raw[a..b],
                                    _ => "",
                                }
                            };
                            let rt = inner_texts(slice("rt"));
                            let base_raw = slice("rubyBase").to_string();
                            let base = inner_texts(&base_raw);
                            // 基底の大きさは rubyBase の中の w:sz から
                            let pt = base_raw
                                .find("<w:sz ")
                                .and_then(|i| {
                                    base_raw[i..].find("w:val=\"").map(|j| i + j + 7)
                                })
                                .and_then(|s0| {
                                    base_raw[s0..].find('"').and_then(|e2| {
                                        base_raw[s0..s0 + e2].parse::<f32>().ok()
                                    })
                                })
                                .map(|h| h / 2.0)
                                .or(size_pt);
                            if !base.is_empty() {
                                if let Some(p) = para.as_mut() {
                                    let mut f2 = fmt.clone();
                                    f2.ruby = (!rt.is_empty()).then_some(rt);
                                    p.push(Run {
                                        text: base,
                                        size_pt: pt,
                                        font: font.clone(),
                                        fmt: f2,
                                    });
                                }
                            }
                        }
                    }
                    b"instrText" => in_instr = true,
                    b"fldChar" => fldchar(attr(&e, "fldCharType").as_deref(),
                        &mut in_field, &mut field_hide, &mut field_instr, &mut field_buf,
                        &mut para, &mut rep, size_pt, &font, &fmt),
                    other => {
                        let _ = other;
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let n = local(e.name().as_ref()).to_vec();
                match n.as_slice() {
                    // 記入欄の設定は空要素で来る(<w:alias w:val="…"/> 等)
                    _ if in_sdtpr && sdt_pr_elem(&n, &e, &mut sdt_now) => {}
                    // gridCol は空要素で来る
                    b"gridCol" => if let Some(b) = stack.last_mut() {
                        if let Some(w) = attr(&e, "w").and_then(|v| v.parse::<f32>().ok()) {
                            b.col_mm.push(twip_mm(w));
                        }
                    },
                    // **空の段落 `<w:p/>`。** 中身の無い段落を Word はこの形で書く。
                    // Start の枝には来ないので、ここで拾わないと**空行がまるごと消える** —
                    // 段落の番号がずれ、保存すると行が詰まる。
                    // (2026-08-10、他人の docx で本文 76 段落中 28 個がこの形だった。
                    //  xlsx の sheetView を Empty の枝でしか読んでいなかったのと**同じ形の穴**)
                    b"p" => {
                        rep.paragraphs += 1;
                        let p = Paragraph {
                            line_spacing: 1.0,
                            runs: vec![Run {
                                text: String::new(),
                                size_pt: None,
                                font: None,
                                fmt: Default::default(),
                            }],
                            ..Default::default()
                        };
                        match stack.last_mut() {
                            Some(b) => b.cell.push(p),
                            None => doc.push_para(p),
                        }
                    }
                    b"br" => if let Some(p) = para.as_mut() {
                        p.push(Run { text: "\n".into(), size_pt, font: font.clone(), fmt: fmt.clone() }) },
                    b"tab" => if let Some(p) = para.as_mut() {
                        p.push(Run { text: "\t".into(), size_pt, font: font.clone(), fmt: fmt.clone() }) },
                    b"sz" if in_rpr => {
                        if let Some(v) = attr(&e, "val") {
                            if let Ok(h) = v.parse::<f32>() { size_pt = Some(h / 2.0); }
                        }
                    }
                    b"rFonts" if in_rpr => {
                        font = attr(&e, "eastAsia")
                            .or_else(|| attr(&e, "ascii"))
                            .or_else(|| attr(&e, "hAnsi"))
                            .filter(|s| !s.is_empty());
                    }
                    // w:val="0"/"false" は「付けない」の意味なので、有無だけで判定しない
                    b"b" if in_rpr => fmt.bold = on(&e),
                    b"i" if in_rpr => fmt.italic = on(&e),
                    b"u" if in_rpr => fmt.underline = attr(&e, "val").as_deref() != Some("none"),
                    b"strike" if in_rpr => fmt.strike = on(&e),
                    b"color" if in_rpr => {
                        fmt.color = attr(&e, "val").filter(|v| !v.is_empty() && v != "auto");
                    }
                    b"vertAlign" if in_rpr => {
                        match attr(&e, "val").as_deref() {
                            Some("superscript") => fmt.superscript = true,
                            Some("subscript") => fmt.subscript = true,
                            _ => {}
                        }
                    }
                    b"highlight" if in_rpr => {
                        fmt.highlight = attr(&e, "val").filter(|v| v != "none");
                    }

                    // 箇条書きは numId で決まる。1 を中黒、2 を段落番号として扱う
                    // (numbering.xml を持たないので、往復できる最小の約束にしてある)
                    b"ilvl" if in_ppr => {
                        ilvl = attr(&e, "val").and_then(|v| v.parse().ok()).unwrap_or(0).min(8);
                    }
                    b"numId" if in_ppr => {
                        list = match attr(&e, "val").as_deref() {
                            Some("2") => ListKind::Number,
                            Some("0") | None => ListKind::None,
                            _ => ListKind::Bullet,
                        };
                    }
                    // 段落のスタイル。見出しと目次の行だけを持つ
                    b"pStyle" if in_ppr => {
                        if let Some(v) = attr(&e, "val") {
                            pstyle = style_of(&v);
                            // 役割を知らないスタイル名も**捨てない** — 原文のまま運ぶ
                            pstyle_id = Some(v);
                        }
                    }
                    // 文字スタイル。名前を運んで返すだけ(定義は styles.xml)
                    b"rStyle" if in_rpr => {
                        fmt.style_id = attr(&e, "val").filter(|v| !v.is_empty());
                    }
                    // スタイル名で見出しと分からなくても、outlineLvl があれば見出し
                    b"outlineLvl" if in_ppr => {
                        if pstyle == ParaStyle::Body {
                            if let Some(n) = attr(&e, "val").and_then(|v| v.parse::<u8>().ok()) {
                                if n < 3 { pstyle = ParaStyle::Heading(n + 1); }
                            }
                        }
                    }
                    b"framePr" if in_ppr => {
                        dropcap = matches!(attr(&e, "dropCap").as_deref(),
                            Some("drop") | Some("margin"));
                    }
                    b"pageBreakBefore" if in_ppr => {
                        page_break_before = on(&e);
                    }
                    b"ind" if in_ppr => {
                        // twip。1段 = 全角2文字 = 10.5pt×2 ≒ 420twip
                        indent = attr(&e, "left")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 420.0).round().clamp(0.0, 20.0) as u8)
                            .unwrap_or(0);
                        // 1行目の字下げは twip のまま(段落を触っても落とさない —
                        // 2026-08-13 に「黙って消える」を実測で踏んだ)
                        first_line = attr(&e, "firstLine")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v as i32)
                            .or_else(|| {
                                attr(&e, "hanging")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -(v as i32))
                            })
                            .unwrap_or(0);
                    }
                    b"spacing" if in_ppr => {
                        // w:line は 240 = 1行
                        line_spacing = attr(&e, "line")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 240.0).clamp(0.5, 5.0))
                            .unwrap_or(1.0);
                    }
                    b"jc" if in_ppr => {
                        if let Some(v) = attr(&e, "val") { align = Align::from_docx(&v); }
                    }
                    // 表の置き方・スタイル名・列幅の固定(w:tblPr の中)
                    b"jc" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.align = attr(&e, "val").map(|v| Align::from_docx(&v));
                    },
                    b"tblStyle" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.style = attr(&e, "val").filter(|v| !v.is_empty());
                    },
                    b"tblLayout" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.fixed_layout = attr(&e, "type").as_deref() == Some("fixed");
                    },
                    // 段落の背景色。fill が色(auto 以外)のときだけ
                    b"shd" if in_ppr => {
                        shade = attr(&e, "fill")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // 段落の囲み枠。辺の別は持たない(あれば囲みとみなす)
                    b"pBdr" if in_ppr => boxed = true,
                    // セル結合(空要素で来るのが普通の形)
                    b"gridSpan" => if stack.last().is_some() {
                        cell_span = attr(&e, "val").and_then(|v| v.parse().ok()).unwrap_or(0);
                    },
                    b"vMerge" => if stack.last().is_some() {
                        cell_vmerge = match attr(&e, "val").as_deref() {
                            Some("restart") => VMerge::Start,
                            _ => VMerge::Continue,
                        };
                    },
                    b"drawing" | b"pict" | b"object" =>
                        rep.note(&format!("w:{}", String::from_utf8_lossy(&n))),
                    // 脚注・文末脚注の印。**模型に持てない** — 本文を作り直すときに
                    // 落ちるので、footnotes.xml だけが行き場を失って残る。
                    // 直せていない物を黙って落とさないために、帳簿には必ず出す
                    // (2026-08-10、genoffice の読み手と突き合わせて分かった。
                    //  空要素で来るのが普通だが、念のため Start の枝にも置いてある —
                    //  xlsx の sheetView を Empty の枝でしか読んでいなかった轍を踏まない)
                    b"footnoteReference" | b"endnoteReference" =>
                        note_mark(&e, &n, &mut para, size_pt, &font, &fmt, &mut rep),
                    // ページの色(空要素で来るのが普通の形)
                    b"background" => {
                        doc.page_color = attr(&e, "color")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // しおり(bookmark)。名前を段落に持ち、保存で振り直す
                    // (範囲は段落単位 — コメントと同じ粒度)。
                    // 相互参照(REF)が指す名前はそのまま残る
                    b"bookmarkStart" => {
                        match (para.is_some(), attr(&e, "name")) {
                            (true, Some(name)) if !name.is_empty() => {
                                para_bookmarks.push(name);
                            }
                            (false, Some(_)) => {
                                rep.note("しおり(段落の外。保存で失われる)");
                            }
                            _ => {}
                        }
                    }
                    b"bookmarkEnd" => {} // 始まり側で持つ。終わりは保存で振り直す
                    // コメント。id が comments.xml で引けたら**段落のコメント**として
                    // 持つ(保存で作り直す)。引けなければ原文を控えて生かす
                    b"commentRangeStart" | b"commentRangeEnd" => {
                        let known = attr(&e, "id").is_some_and(|i| cmts.contains_key(&i));
                        if !known
                            && para.is_some() {
                                let raw = &xml[start_pos..last_pos];
                                anchors.push(raw.trim().to_string());
                                rep.note("しおり・コメントの印(段落の頭に寄るが、保存で残る)");
                            }
                    }
                    b"commentReference" => {
                        match attr(&e, "id").and_then(|i| cmts.get(&i)) {
                            Some(c) => para_comments.push(c.clone()),
                            None => {
                                if para.is_some() {
                                    let raw = &xml[start_pos..last_pos];
                                    anchors.push(format!("<w:r>{}</w:r>", raw.trim()));
                                    rep.note("しおり・コメントの印(段落の頭に寄るが、保存で残る)");
                                }
                            }
                        }
                    }
                    // fldChar は空要素で来るのが普通の形
                    b"fldChar" => fldchar(attr(&e, "fldCharType").as_deref(),
                        &mut in_field, &mut field_hide, &mut field_instr, &mut field_buf,
                        &mut para, &mut rep, size_pt, &font, &fmt),
                    b"fldSimple" => {
                        // 空の fldSimple(中身なし)。持てる命令なら印だけ置く
                        let instr = attr(&e, "instr").unwrap_or_default();
                        if let Some(mark) = field_mark(&instr) {
                            if let Some(p) = para.as_mut() {
                                p.push(Run { text: mark.to_string(), size_pt,
                                             font: font.clone(), fmt: fmt.clone() });
                            }
                        } else {
                            rep.note(&format!("フィールド({})", instr.trim()));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    cur.push_str(&t.unescape().unwrap_or_default());
                } else if in_instr {
                    field_instr.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                let n = local(e.name().as_ref()).to_vec();
                match n.as_slice() {
                    b"sdtPr" => in_sdtpr = false,
                    b"sdt" => {
                        sdt_depth = sdt_depth.saturating_sub(1);
                        if sdt_depth == 0 {
                            sdt_cur = None;
                            sdt_now = None;
                        }
                    }
                    b"t" => {
                        in_text = false;
                        if field_hide {
                            // フィールドの計算済みの見た目。REF はここが「見えている値」
                            // になるので控える(PAGE 等では捨てられる)
                            field_buf.push_str(&std::mem::take(&mut cur));
                        } else if !cur.is_empty() {
                            if let Some(p) = para.as_mut() {
                                p.push(Run { text: std::mem::take(&mut cur), size_pt, font: font.clone(), fmt: fmt.clone() });
                            }
                        }
                    }
                    b"instrText" => in_instr = false,
                    b"rPr" => in_rpr = false,
                    b"hyperlink" => cur_link = None,
                    b"pPr" => in_ppr = false,
                    b"tblPr" => in_tblpr = false,
                    b"p" => {
                        if let Some(runs) = para.take() {
                            rep.runs += runs.len();
                            rep.paragraphs += 1;
                            let mut p = Paragraph { align, anchors: std::mem::take(&mut anchors),
                                sect: para_sect.take(),
                                images: std::mem::take(&mut images),
                                comments: std::mem::take(&mut para_comments),
                                bookmarks: std::mem::take(&mut para_bookmarks),
                                page_break_before, list,
                                // 深さ: w:ind(直接指定)が無ければ w:ilvl から
                                indent: indent.max(ilvl),
                                first_line_twips: first_line,
                                line_spacing,
                                style: pstyle,
                                style_id: pstyle_id.take(),
                                shade: shade.take(), boxed,
                                dropcap: false,
                                images_new: Vec::new(),
                                runs: if runs.is_empty() {
                                vec![Run { text: String::new(), size_pt: None, font: None, fmt: Default::default() }]
                            } else { runs } };
                            // ドロップキャップの枠の段落は、次の段落の頭に合流する
                            if dropcap && !p.runs.iter().all(|r| r.text.is_empty()) {
                                pending_cap = Some(p.runs);
                            } else {
                                if let Some(mut cap) = pending_cap.take() {
                                    // 頭の字の大きさは本文に合わせる(2.8倍は組むとき掛かる。
                                    // 読んだ大きさのまま持つと保存のたびに育つ)
                                    let body_pt = p.runs.first().and_then(|r| r.size_pt);
                                    for r in &mut cap {
                                        r.size_pt = body_pt;
                                    }
                                    cap.extend(p.runs);
                                    p.runs = cap;
                                    p.dropcap = true;
                                }
                                // 表のセルの中なら、そのセルへ。外なら本文へ
                                match stack.last_mut() {
                                    Some(b) => b.cell.push(p),
                                    None => doc.push_para(p),
                                }
                            }
                        }
                    }
                    b"tc" => if let Some(b) = stack.last_mut() {
                        // 枠の段落だけで終わったセルは、そのまま段落として置く
                        if let Some(cap) = pending_cap.take() {
                            b.cell.push(Paragraph {
                                dropcap: true,
                                line_spacing: 1.0,
                                runs: cap,
                                ..Default::default()
                            });
                        }
                        let paras = std::mem::take(&mut b.cell);
                        b.row.push(Cellbox {
                            paragraphs: paras,
                            col_span: cell_span,
                            v_merge: cell_vmerge,
                        });
                        cell_span = 0;
                        cell_vmerge = VMerge::None;
                    },
                    b"tr" => if let Some(b) = stack.last_mut() {
                        let row = std::mem::take(&mut b.row);
                        b.rows.push(row);
                    },
                    b"tbl" => {
                        if let Some(b) = stack.pop() {
                            let tb = Table {
                                rows: b.rows,
                                col_mm: b.col_mm,
                                style: b.style,
                                align: b.align,
                                fixed_layout: b.fixed_layout,
                            };
                            if stack.is_empty() {
                                doc.blocks.push(Block::Table(tb));
                            } else {
                                // 入れ子の表は v0 では本文の流れに出す(報告つき)
                                rep.note("入れ子の表(親セルの外に出した)");
                                doc.blocks.push(Block::Table(tb));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }
    (doc, rep)
}