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
    // **属性の並び順を当てにしません**(2026-08-31 発注者)。前は `Id="` を
    // 見つけてから**その後ろ**で `Target="` を探していました。内閣府の様式
    // (document_4.docx)は `Type` `Target` `Id` の順に書いてあるので、次の
    // 関係の `Target` を拾って対応がずれ、**絵が3枚とも消えていました**。
    // すぐ下の `bui` は最初から要素ごとに区切って読んでいます
    let mut targets: std::collections::BTreeMap<String, String> = Default::default();
    for r in rels.split("<Relationship").skip(1) {
        // 1つの要素の中だけを見ます
        let r = &r[..r.find('>').unwrap_or(r.len())];
        let hiku = |k: &str| -> Option<String> {
            let i = r.find(k)? + k.len();
            let e = r[i..].find('"')? + i;
            Some(r[i..e].to_string())
        };
        if let (Some(id), Some(t)) = (hiku("Id=\""), hiku("Target=\"")) {
            targets.insert(id, t);
        }
    }
    // **部品の在り処は関係で引きます。** `word/styles.xml` は慣習で、
    // 決まりではありません。内閣府の様式(document_4.docx)は
    // `word/styles2.xml` `word/settings2.xml` `word/theme/theme11.xml` を
    // 使っていて、名前で探していたスタイル・設定・テーマが**1つも
    // 読めていません**でした(2026-08-30)
    let bui = |kind: &str, kisoku: &str| -> String {
        let mata = format!("/{kind}\"");
        for r in rels.split("<Relationship").skip(1) {
            let Some(ti) = r.find("Type=\"") else { continue };
            let Some(te) = r[ti + 6..].find('"') else { continue };
            if !r[ti + 6..ti + 6 + te].ends_with(&mata[1..mata.len() - 1]) {
                continue;
            }
            let Some(gi) = r.find("Target=\"") else { continue };
            let Some(ge) = r[gi + 8..].find('"') else { continue };
            let t = &r[gi + 8..gi + 8 + ge];
            // `/word/styles.xml` のような絶対の書き方も来ます
            return if let Some(x) = t.strip_prefix('/') {
                x.to_string()
            } else {
                format!("word/{t}")
            };
        }
        kisoku.to_string()
    };
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
    if let Ok(mut f) = zip.by_name(&bui("styles", "word/styles.xml")) {
        let _ = f.read_to_string(&mut styxml);
    }

    // **箇条書きの印**(numbering.xml)。`numId` と段から `w:lvlText` を引きます。
    // 前は「numId 1 は中黒、2 は番号」の決め打ちで、文書が決めた印を
    // 見ていませんでした(2026-08-31。内閣府の調査票の `○` が9か所)
    let mut numxml = String::new();
    if let Ok(mut f) = zip.by_name(&bui("numbering", "word/numbering.xml")) {
        let _ = f.read_to_string(&mut numxml);
    }
    let shirushi = num_markers(&numxml);

    // 設定(settings.xml)。欧文ハイフネーションの旗を読む
    let mut sxml = String::new();
    if let Ok(mut f) = zip.by_name(&bui("settings", "word/settings.xml")) {
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
    let (mut doc, mut rep) = parse_document_rels_num(&xml, &media, &cmap, &targets, &shirushi);
    // このアプリのペン(joink)は原文控えから筆へ読み戻す
    extract_ink(&mut doc);
    extract_shapes(&mut doc);
    if !styxml.is_empty() {
        doc.styles = parse_styles_num(&styxml, &shirushi);
        hyou_no_kei(&mut doc, &styxml);
    }
    tblind_wo_naosu(&mut doc, &sxml);
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
            // **この部品の図形も運びます**(2026-09-01)。紙の飾り枠は
            // ヘッダーに置かれます。段落の控えから集めます
            hf.anchors = hdoc
                .paragraphs()
                .flat_map(|p| p.anchors.iter().cloned())
                .filter(|a| a.contains("<w:drawing") || a.contains("<w:pict"))
                .collect();
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
    if let Ok(mut f) = zip.by_name(&bui("styles", "word/styles.xml")) {
        let _ = f.read_to_string(&mut styles);
    }
    // **docx を読むときの層0の既定。**
    //
    // 規格(ECMA-376 §17.7.2)の層の並びに「アプリの既定」はありません。
    // どこにも書いていないときに読み手が使う値です。LibreOffice は
    // 10pt を置き、**言語では変えません**(`StyleSheetTable` の作りはじめと、
    // tdf#87533「LibreOffice の既定は言語で変わるので、決められた値で読む」)。
    // うちも同じにします。`DEFAULT_PT`(10.5)は自分で作る文書の既定で、
    // docx を読む道では使いません(2026-09-03 発注者)
    doc.size_pt = Some(10.0);
    // **テーマの配色。** 図形の色はここの名前で書いてあります
    {
        let mut th = String::new();
        if let Ok(mut f) = zip.by_name(&bui("theme", "word/theme/theme1.xml")) {
            let _ = f.read_to_string(&mut th);
        }
        doc.theme_colors = crate::theme::clr_scheme(&th);
    }
    if let Some(i) = styles.find("docDefaults") {
        // **層1。** この節が言うことは、スタイルより下・層0より上です。
        // 字の大きさ・段落後の空き・行間を読みます。python-docx の型紙は
        // 11pt・段落後 10pt・行間 1.15 を書きます
        if let Some(e) = styles[i..].find("</w:docDefaults>").map(|e| i + e) {
            let naka = &styles[i..e];
            let tag = |from: &str, name: &str| -> Option<String> {
                let n = from.find(name)?;
                let k = n + from[n..].find('>')?;
                Some(from[n..k].to_string())
            };
            // 札の中の属性を引く(この節だけの小さな道具)
            let zoku = |t: &str, key: &str| -> Option<String> {
                let pat = format!("{key}=\"");
                let i = t.find(&pat)? + pat.len();
                let e = t[i..].find('"')? + i;
                Some(t[i..e].to_string())
            };
            // 字の大きさは `w:rPrDefault` の中の `w:sz`(2分の1 pt)
            if let Some(rp) = naka.find("<w:rPrDefault").and_then(|n| {
                naka[n..].find("</w:rPrDefault>").map(|e| &naka[n..n + e])
            }) {
                if let Some(t) = tag(rp, "<w:sz ") {
                    if let Some(Ok(h)) = zoku(&t, "w:val").map(|v| v.parse::<f32>()) {
                        doc.size_pt = Some(h / 2.0);
                    }
                }
            }
            // 段落の空きと行間は `w:pPrDefault` の中の `w:spacing`
            if let Some(pp) = naka.find("<w:pPrDefault").and_then(|n| {
                naka[n..].find("</w:pPrDefault>").map(|e| &naka[n..n + e])
            }) {
                if let Some(t) = tag(pp, "<w:spacing") {
                    // twip の 20 分の1が pt
                    if let Some(Ok(v)) = zoku(&t, "w:after").map(|v| v.parse::<f32>()) {
                        doc.space_after_pt = Some(v / 20.0);
                    }
                    // 240 が1行(docx の決め)。`w:lineRule` が auto のときだけ倍率
                    if zoku(&t, "w:lineRule").as_deref() == Some("auto") {
                        if let Some(Ok(v)) = zoku(&t, "w:line").map(|v| v.parse::<f32>()) {
                            doc.line_spacing = Some(v / 240.0);
                        }
                    }
                }
            }
        }
        let head = &styles[i..(i + 600).min(styles.len())];
        // **書体は <w:rFonts> の中だけから読む。** docDefaults には
        // <w:lang w:val="en-US" w:eastAsia="en-US"/> のような**言語**の指定も
        // 並んでいて、頭からの文字列検索だと言語の札を書体として拾う
        // (python-docx の既定の styles.xml で実際に踏んだ — 画面に
        // 「書体『en-US』が無い」と出ていた)
        let rfonts = head
            .find("<w:rFonts")
            .and_then(|j| head[j..].find('>').map(|e| &head[j..j + e]));
        if let Some(tag) = rfonts {
            for key in ["w:eastAsia=\"", "w:ascii=\""] {
                if let Some(j) = tag.find(key) {
                    let s = j + key.len();
                    if let Some(e) = tag[s..].find('"') {
                        doc.font = Some(tag[s..s + e].to_string());
                        break;
                    }
                }
            }
            // 名前が直に無く、テーマ名(minorEastAsia など)で書いてある docx は
            // theme1.xml の fontScheme を引いて名前にする。python-docx の既定が
            // この形(w:asciiTheme="minorHAnsi")
            if doc.font.is_none() && tag.contains("Theme=\"") {
                let mut theme = String::new();
                if let Ok(mut f) = zip.by_name(&bui("theme", "word/theme/theme1.xml")) {
                    let _ = f.read_to_string(&mut theme);
                }
                // 本文の既定は minor の組。major は見出し用
                let group = if tag.contains("Theme=\"major") {
                    "<a:majorFont>"
                } else {
                    "<a:minorFont>"
                };
                if let Some(g) = theme.find(group) {
                    let sect = &theme[g..(g + 400).min(theme.len())];
                    // 日本語の書体(<a:ea>)を先に。Office の既定のテーマは
                    // <a:ea typeface=""/> が**空**で、日本語は script="Jpan" の
                    // 表で持つ(本物の python-docx の出力で確かめた)。
                    // どちらも無ければ欧文(<a:latin>)
                    for key in [
                        "<a:ea typeface=\"",
                        "<a:font script=\"Jpan\" typeface=\"",
                        "<a:latin typeface=\"",
                    ] {
                        if let Some(j) = sect.find(key) {
                            let s = j + key.len();
                            if let Some(e) = sect[s..].find('"') {
                                if e > 0 {
                                    doc.font = Some(sect[s..s + e].to_string());
                                    break;
                                }
                            }
                        }
                    }
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
    // **読みでは run を繋ぎません**(2026-09-01)。原文の分かれ目は
    // 原文の情報です。繋ぐのは編集の側([`kumihan::Document::heal_runs`])。
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
    /// 表スタイルのどの条件を効かせるか(`w:tblLook`)
    look: kumihan::TblLook,
    /// セルの中の余白(`w:tblCellMar`。mm。上右下左)
    cell_mar_mm: Option<[f32; 4]>,
    /// 表の置き方(tblPr の w:jc)
    align: Option<Align>,
    /// 列幅の固定(w:tblLayout type="fixed")
    fixed_layout: bool,
    /// 行の高さ(mm)。w:trPr の w:trHeight から
    row_mm: Vec<f32>,
    /// 罫線の指定(w:tblBorders)。**書いてなければ None** で、
    /// そのときは今までどおり四方に引きます
    borders: Option<kumihan::TableBorders>,
    /// 表の幅を本文の幅の割合で言うとき(`w:tblW w:type="pct"`)の%
    width_pct: Option<f32>,
    /// 表の左のインデント(`w:tblInd`)の twip。**原文のまま**持ちます。
    /// セルの余白を引く補正は、設定(compatibilityMode)を読める所でします
    ind_twips: Option<f32>,
    /// 見出しの行(`w:trPr/w:tblHeader`)が最初の行に付いていたか
    header_row: bool,
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
    // 包む `<w:r>` は新しく建てる外側の要素なので、原文の自前宣言は内側に残る。
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
    Some(kumihan::InlineImage { bytes, w_mm: cx / 36000.0, h_mm: cy / 36000.0, tex, src: None,
        off: 0 })
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

    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
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

/// sectPr の `w:type`(`<w:type w:val="continuous"/>` の val)。無ければ docx の
/// 既定 `nextPage`。この値は、その sectPr で終わる節の始め方です。
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
    parse_styles_num(xml, &Default::default())
}

/// **箇条書きの印の表つき。** スタイルの `w:numPr/w:numId` を
/// `numbering.xml` で引いて、中黒か番号かと印の字にします。
///
/// python-docx の `add_paragraph(style="List Bullet")` は本文に `w:numPr` を
/// 書かないので、これを読まないと箇条書きがただの段落になります(2026-09-03)
pub(super) fn parse_styles_num(
    xml: &str,
    shirushi: &std::collections::BTreeMap<(u32, u8), (String, bool)>,
) -> Vec<kumihan::StyleInfo> {
    /// スタイルの `w:rPr` と `w:pPr` から見た目を読む。**読むだけ**です。
/// スタイルの段落の見た目(`w:pPr` の中)。読めない物は「言わない」のまま
fn style_para(
    body: &str,
    shirushi: &std::collections::BTreeMap<(u32, u8), (String, bool)>,
) -> kumihan::StyleParaLook {
    let val = |tag: &str| -> Option<String> {
        let t = format!("<{tag}");
        body.find(&t).map(|n| {
            let e = body[n..].find('>').map(|e| n + e).unwrap_or(body.len());
            attr_of(&body[n..e], "w:val")
        })
    };
    let spacing = |key: &str| -> Option<f32> {
        let n = body.find("<w:spacing")?;
        let e = body[n..].find('>').map(|e| n + e)?;
        let v = attr_of(&body[n..e], key);
        // twip の 20 分の1が pt
        v.parse::<f32>().ok().map(|t| t / 20.0)
    };
    kumihan::StyleParaLook {
        align: val("w:jc").as_deref().and_then(align_of),
        space_before_pt: spacing("w:before"),
        space_after_pt: spacing("w:after"),
        // 行間は 240 が1行(docx の決め)
        line_spacing: {
            let n = body.find("<w:spacing");
            n.and_then(|n| body[n..].find('>').map(|e| n + e))
                .map(|e| attr_of(&body[n.unwrap()..e], "w:line"))
                .and_then(|v| v.parse::<f32>().ok())
                .map(|l| l / 240.0)
        },
        indent: ind(body, "w:left").map(|t| (t / 480.0).round().clamp(0.0, 9.0) as u8),
        first_line_twips: ind(body, "w:firstLine")
            .or_else(|| ind(body, "w:hanging").map(|v| -v))
            .map(|v| v as i32),
        // **箇条書き。** `w:numPr` の `w:numId` を印の表で引きます
        list: num_of(body).map(|n| {
            match shirushi.get(&(n, 0)) {
                Some((_, kazu)) => if *kazu { kumihan::ListKind::Number } else { kumihan::ListKind::Bullet },
                // 表が無い docx は numId の決め打ち(本文の側と同じ約束)
                None => if n == 2 { kumihan::ListKind::Number } else { kumihan::ListKind::Bullet },
            }
        }),
        list_text: num_of(body).and_then(|n| shirushi.get(&(n, 0)).map(|(t, _)| t.clone())),
        // **段落の罫線。** 本文の `w:pBdr` と同じ辺を読みます
        border: pbdr_of(body),
        // 同じスタイルが続く間は空きを入れない
        contextual_spacing: body.contains("<w:contextualSpacing").then_some(true),
    }
}


/// **表のスタイルが言う書式を読む**(docx の `w:type="table"` のスタイル)。
///
/// python-docx の `add_table(style=…)` は本文に色も罫線も書きません。
/// 全部ここにあります。`w:tblStylePr` が条件ごとの書式で、`w:type` が
/// `firstRow`(見出し行)・`band1Horz`(1行おきの帯)などです(2026-09-03)。
fn table_style(body: &str) -> kumihan::TableStyleLook {
    let mut t = kumihan::TableStyleLook::default();
    // 条件つきの塊を先に切り分けます。残りが表全体の分です
    let mut zentai = String::new();
    let mut at = 0usize;
    let mut jouken: Vec<(String, &str)> = Vec::new();
    while let Some(k) = body[at..].find("<w:tblStylePr ") {
        let i = at + k;
        zentai.push_str(&body[at..i]);
        let e = body[i..]
            .find("</w:tblStylePr>")
            .map(|e| i + e + 15)
            .unwrap_or(body.len());
        let atama = body[i..].find('>').map(|q| i + q).unwrap_or(i);
        let na = attr_of(&body[i..atama], "w:type");
        jouken.push((na, &body[i..e]));
        at = e;
    }
    zentai.push_str(&body[at..]);
    t.base = table_cond(&zentai);
    for (na, blk) in jouken {
        let c = table_cond(blk);
        match na.as_str() {
            "firstRow" => t.first_row = c,
            "lastRow" => t.last_row = c,
            "firstCol" => t.first_col = c,
            "lastCol" => t.last_col = c,
            "band1Horz" => t.band1_h = c,
            "band2Horz" => t.band2_h = c,
            "band1Vert" => t.band1_v = c,
            "band2Vert" => t.band2_v = c,
            // 四隅(nwCell など)は、行と列の条件で決まるので持ちません
            _ => {}
        }
    }
    let kazu = |na: &str| -> u8 {
        // **その札の中だけ**を見ます。頭から探すと、手前の別の札の
        // `w:val` を拾います
        zentai
            .find(na)
            .and_then(|k| zentai[k..].find('>').map(|e| (k, k + e)))
            .map(|(k, e)| attr_of(&zentai[k..e], "w:val"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    t.row_band = kazu("<w:tblStyleRowBandSize");
    t.col_band = kazu("<w:tblStyleColBandSize");
    t.cell_mar_mm = cell_mar(&zentai);
    t.borders = tbl_borders(&zentai);
    t
}

/// 条件1つ分の書式(塗り・太字・字の色)
fn table_cond(blk: &str) -> kumihan::TableCond {
    let mut c = kumihan::TableCond::default();
    // 段落の書式。`w:pPr` の中だけを見ます
    if let Some(k) = blk.find("<w:pPr>") {
        let e = blk[k..].find("</w:pPr>").map(|e| k + e).unwrap_or(blk.len());
        c.para = style_para(&blk[k..e], &Default::default());
    }
    // 塗りは `w:tcPr` の中の `w:shd w:fill`
    if let Some(k) = blk.find("<w:shd ") {
        let e = blk[k..].find('>').map(|e| k + e).unwrap_or(blk.len());
        let v = attr_of(&blk[k..e], "w:fill");
        if !v.is_empty() && v != "auto" {
            c.shade = Some(v);
        }
    }
    // 字は `w:rPr` の中
    if let Some(k) = blk.find("<w:rPr>") {
        let e = blk[k..].find("</w:rPr>").map(|e| k + e).unwrap_or(blk.len());
        let rpr = &blk[k..e];
        let look = style_look(rpr);
        c.bold = look.bold;
        c.color = look.color;
    }
    c
}

/// `w:tblCellMar` を mm(上右下左)で
fn cell_mar(body: &str) -> Option<[f32; 4]> {
    let k = body.find("<w:tblCellMar>")?;
    let e = body[k..].find("</w:tblCellMar>").map(|e| k + e)?;
    let naka = &body[k..e];
    let hen = |na: &str| -> f32 {
        // **その辺の札の中だけ**を見ます(上の `kazu` と同じ理由)
        naka.find(na)
            .and_then(|p| naka[p..].find('>').map(|q| (p, p + q)))
            .map(|(p, q)| attr_of(&naka[p..q], "w:w"))
            .and_then(|v| v.parse::<f32>().ok())
            // twip → mm
            .map(|v| v * 25.4 / 1440.0)
            .unwrap_or(0.0)
    };
    Some([hen("<w:top"), hen("<w:right"), hen("<w:bottom"), hen("<w:left")])
}

/// スタイルの `w:tblBorders`(辺だけ)
fn tbl_borders(body: &str) -> Option<kumihan::TableBorders> {
    let k = body.find("<w:tblBorders>")?;
    let e = body[k..].find("</w:tblBorders>").map(|e| k + e)?;
    let naka = &body[k..e];
    let mut b = kumihan::TableBorders::nashi();
    for (tag, at) in [
        ("<w:top", 0u8), ("<w:left", 1), ("<w:bottom", 2), ("<w:right", 3),
        ("<w:insideH", 4), ("<w:insideV", 5),
    ] {
        let Some(p) = naka.find(tag) else { continue };
        let q = naka[p..].find('>').map(|q| p + q).unwrap_or(naka.len());
        let seg = &naka[p..q];
        if !matches!(seg.as_bytes().get(tag.len()), Some(b' ') | Some(b'/') | Some(b'>')) {
            continue;
        }
        let v = attr_of(seg, "w:val");
        if v == "nil" || v == "none" {
            continue;
        }
        match at {
            0 => b.top = true,
            1 => b.left = true,
            2 => b.bottom = true,
            3 => b.right = true,
            4 => b.inside_h = true,
            _ => b.inside_v = true,
        }
    }
    Some(b)
}

/// スタイルの `w:pPr/w:numPr/w:numId` の値
fn num_of(body: &str) -> Option<u32> {
    let n = body.find("<w:numPr")?;
    let k = n + body[n..].find("<w:numId")?;
    let e = k + body[k..].find('>')?;
    attr_of(&body[k..e], "w:val").parse().ok()
}

/// **スタイルの `w:pBdr` を読む。** 本文の側は読み手の流れの中で組み立てますが、
/// スタイルは字の並びしか無いので、こちらで同じ辺を拾います。
///
/// python-docx の既定の型紙は、題(`Title`)の下の線をここに書きます
/// (本文には1文字もありません)。2026-09-03。
fn pbdr_of(body: &str) -> Option<kumihan::ParaBorder> {
    let n = body.find("<w:pBdr")?;
    let owari = body[n..].find("</w:pBdr>").map(|e| n + e).unwrap_or(body.len());
    let naka = &body[n..owari];
    let mut b = kumihan::ParaBorder::default();
    for (tag, at) in [
        ("<w:top", 0u8), ("<w:bottom", 1), ("<w:left", 2), ("<w:right", 3), ("<w:between", 4),
    ] {
        let Some(k) = naka.find(tag) else { continue };
        let e = naka[k..].find('>').map(|e| k + e).unwrap_or(naka.len());
        let seg = &naka[k..e];
        // 同じ字で始まる別の名前(`<w:topLinePunct` など)を拾わない
        if !matches!(seg.as_bytes().get(tag.len()), Some(b' ') | Some(b'/') | Some(b'>')) {
            continue;
        }
        let v = attr_of(seg, "w:val");
        if v == "none" || v == "nil" {
            continue;
        }
        if let Ok(x) = attr_of(seg, "w:space").parse::<f32>() {
            b.space_pt = b.space_pt.max(x);
        }
        if let Ok(x) = attr_of(seg, "w:sz").parse::<f32>() {
            b.w_pt = b.w_pt.max(x / 8.0);
        }
        match at {
            0 => b.top = true,
            1 => b.bottom = true,
            2 => b.left = true,
            3 => b.right = true,
            _ => b.between = true,
        }
    }
    b.aru().then_some(b)
}

/// `w:ind` の値(twip)
fn ind(body: &str, key: &str) -> Option<f32> {
    let n = body.find("<w:ind")?;
    let e = body[n..].find('>').map(|e| n + e)?;
    attr_of(&body[n..e], key).parse().ok()
}

/// `w:jc` の値を揃えへ
fn align_of(v: &str) -> Option<kumihan::Align> {
    Some(match v {
        "left" | "start" => kumihan::Align::Left,
        "center" => kumihan::Align::Center,
        "right" | "end" => kumihan::Align::Right,
        "both" | "distribute" => kumihan::Align::Justify,
        _ => return None,
    })
}

fn style_look(body: &str) -> kumihan::StyleLook {
    let mut l = kumihan::StyleLook::default();
    // 三択を守ります。`<w:b/>` は入、`<w:b w:val="0"/>` は切、無ければ言わない
    //
    // **同じ字で始まる別の名前を飛ばして、次を探します**(2026-09-03)。
    // `<w:b` は `<w:basedOn` にも、`<w:u` は `<w:uiPriority` にも当たります。
    // 前は最初に当たった所で打ち切っていたので、`w:basedOn` を持つスタイルの
    // 太字が1つも読めていませんでした。内閣府の面談の記録
    // (document_4.docx)の「議論項目」は heading 2 で、その定義は
    // `<w:basedOn w:val="a1"/>` の後ろに `<w:b/>` を書いています。
    // 色と大きさは別の名前で引くので効いていて、太字だけが落ちていました
    let flag = |tag: &str| -> Option<bool> {
        let open = format!("<w:{tag}");
        let mut from = 0usize;
        while let Some(k) = body[from..].find(&open) {
            let i = from + k;
            let end = body[i..].find('>').map(|e| i + e + 1)?;
            let seg = &body[i..end];
            match seg.as_bytes().get(open.len()) {
                Some(b'/') | Some(b'>') | Some(b' ') => {
                    return Some(!matches!(
                        attr_of(seg, "w:val").as_str(),
                        "0" | "false" | "none"
                    ));
                }
                _ => from = i + open.len(),
            }
        }
        None
    };
    l.bold = flag("b");
    l.italic = flag("i");
    l.strike = flag("strike");
    l.underline = flag("u");
    let val_of = |tag: &str| -> Option<String> {
        let open = format!("<w:{tag} ");
        let i = body.find(&open)?;
        let seg = &body[i..body[i..].find('>').map(|e| i + e + 1)?];
        let v = attr_of(seg, "w:val");
        (!v.is_empty()).then_some(v)
    };
    l.color = val_of("color").filter(|c| c != "auto");
    l.fill = body
        .find("<w:shd ")
        .and_then(|i| body[i..].find('>').map(|e| &body[i..i + e + 1]))
        .map(|seg| attr_of(seg, "w:fill"))
        .filter(|c| !c.is_empty() && c != "auto");
    l.size_pt = val_of("sz").and_then(|v| v.parse::<f32>().ok()).map(|h| h / 2.0);
    l.font = body
        .find("<w:rFonts ")
        .and_then(|i| body[i..].find('>').map(|e| &body[i..i + e + 1]))
        .map(|seg| attr_of(seg, "w:ascii"))
        .filter(|f| !f.is_empty());
    l
}

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
            // **見た目は読むだけ**です(2026-08-27)。保存では原本の
            // styles.xml を据え置くので、ここで読んだ物は書き戻しません。
            // 読むのは「設定したのに開き直すと None」を無くすためです —
            // ファイルには残っているのに見えないと、失われたように見えます
            // 定義の性質(元になるスタイル・一覧への出し方・順)も読みます。
            // **読むだけでは足りません** — 触った物は保存で書き戻します
            // (2026-08-28。連載の第3回がここを題材にしています)
            let val = |tag: &str| -> Option<String> {
                let t = format!("<{tag}");
                body.find(&t).map(|n| {
                    let e = body[n..].find('>').map(|e| n + e).unwrap_or(body.len());
                    attr_of(&body[n..e], "w:val")
                })
            };
            // `<w:semiHidden/>` のように val を書かない形は「入」です
            let flag = |tag: &str| -> bool {
                match val(tag) {
                    None => false,
                    Some(v) => !matches!(v.as_str(), "0" | "false"),
                }
            };
            out.push(kumihan::StyleInfo {
                id,
                name,
                kind,
                look: style_look(body),
                based_on: val("w:basedOn").filter(|v| !v.is_empty()),
                hidden: flag("w:semiHidden"),
                unhide_when_used: flag("w:unhideWhenUsed"),
                locked: flag("w:locked"),
                quick_style: flag("w:qFormat"),
                default: matches!(attr_of(head, "w:default").as_str(), "1" | "true"),
                priority: val("w:uiPriority").and_then(|v| v.parse().ok()),
                para: style_para(body, shirushi),
                table: table_style(body),
            });
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
        "quote" | "blockquote" | "引用" | "引用文" => ParaStyle::Quote,
        "title" | "表題" => ParaStyle::Title,
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
        // w14:checkbox。tag の jo:radio が先に読めていればラジオのまま残す
        b"checkbox" => {
            let s = sd.get_or_insert_with(Default::default);
            if s.kind != K::Radio {
                s.kind = K::Checkbox;
            }
        }
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
    parse_document_num(xml, media, cmts, &Default::default())
}

/// `shirushi` は `numbering.xml` の印の表([`num_markers`])。
/// 空なら今までどおり `numId` の決め打ちで箇条書きの種類を決めます
pub(super) fn parse_document_num(
    xml: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    cmts: &std::collections::BTreeMap<String, Comment>,
    shirushi: &std::collections::BTreeMap<(u32, u8), (String, bool)>,
) -> (Document, Report) {
    parse_document_rels_num(xml, media, cmts, &Default::default(), shirushi)
}

/// 関係と、箇条書きの印の表([`num_markers`])つき。ここが本体です
pub(super) fn parse_document_rels_num(
    xml: &str,
    media: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    cmts: &std::collections::BTreeMap<String, Comment>,
    targets: &std::collections::BTreeMap<String, String>,
    shirushi: &std::collections::BTreeMap<(u32, u8), (String, bool)>,
) -> (Document, Report) {
    // **BOM をここで外します。** quick-xml は位置を BOM の後ろから数えるのに、
    // こちらの文字列には残っているので、原文を切り出すと3バイトずれます。
    // 内閣府の document_4 は BOM 付きで、絵を控える所が `</w:drawi` で
    // 切れ、開いて保存しただけで壊れた XML になっていました(2026-09-01)。
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);

    let mut doc = Document::default();
    let mut rep = Report::default();
    let mut stack: Vec<TblBuild> = Vec::new();

    let mut para: Option<Vec<Run>> = None;
    // いま読んでいるセルの結合(w:tcPr の gridSpan / vMerge)
    let mut cell_span = 0u8;
    let mut cell_vmerge = VMerge::None;
    // セルの縦位置。**docx の既定は上揃え**(表計算の既定の下揃えとは違う)
    let mut cell_valign = book::VAlign::Top;
    // **セルの塗り**(`w:tcPr` の中の `w:shd w:fill`)。段落の塗りとは別です
    let mut cell_shade: Option<String> = None;
    let mut in_tcpr = false;
    // **罫線の辺の名前は余白にも出ます**(w:tcMar の w:top など)。
    // どの囲みの中に居るかを覚えてから読みます(2026-08-30)
    let mut in_tbl_borders = false;
    let mut in_tc_borders = false;
    let mut cell_borders = kumihan::CellBorders::default();
    // **セルだけの余白**(`w:tcPr/w:tcMar`。mm。上右下左)。
    // 罫線と同じ辺の名前で来るので、どの囲みの中かを覚えて読みます
    let mut in_tc_mar = false;
    let mut in_tbl_cell_mar = false;
    let mut cell_mar: Option<[f32; 4]> = None;
    // セルの幅いっぱいに字を配るか(`w:tcPr/w:tcFitText`)
    let mut cell_fit = false;
    // 行の高さ(twip)。w:trPr の w:trHeight
    let mut row_twips: Option<u32> = None;
    // その行が見出しの行か(`w:trPr/w:tblHeader`)
    let mut row_header = false;
    // **無指定は None のまま持つ。** ここで数を入れると、往復で
    // 「10.5pt 指定」が焼き付く(2026-08-13、本家 python-docx で発覚)
    let mut size_pt: Option<f32> = None;
    // **書体は文書の設定**。docx が w:rFonts で持っているものを捨てない
    let mut font: Option<String> = None;
    // 文字の書式(w:rPr)と段落の揃え(w:jc)。読んで捨てると開き直したとき消える
    let mut fmt = CharFormat::default();
    let mut align = Align::default();
    let mut align_itta = false;
    let mut tab_stops: Vec<i32> = Vec::new();
    // 箇条書き・インデント・行間(w:numPr / w:ind / w:spacing)
    let mut list = ListKind::default();
    // 文書が決めた箇条書きの印(numbering.xml の w:lvlText)
    let mut list_text: Option<String> = None;
    let mut indent = 0u8;
    let mut first_line = 0i32; // w:ind の firstLine(正)/ hanging(負)。twip のまま持つ
    let mut first_line_chars: Option<f32> = None;
    let mut left_twips = 0i32; // w:ind の left。段数と違って丸めない(2026-08-30)
    let mut line_spacing = 0.0f32;
    let mut line_pt: Option<(f32, bool)> = None;
    let mut space_before_pt = 0.0f32;
    let mut space_after_pt = 0.0f32;
    let mut page_break_before = false;
    // 次の段落を新しい紙から始めるか(run の中の `<w:br w:type="page"/>`)
    let mut tsugi_kaipeji = false;
    // 段落の背景色(w:shd)と囲み枠(w:pBdr)
    let mut shade: Option<String> = None;
    let mut boxed = false;
    let mut in_pbdr = false;
    let mut para_border = kumihan::ParaBorder::default();
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
                    b"tblBorders" => {
                        in_tbl_borders = true;
                        // 書いてある辺だけ引きます。まず全部消してから足します
                        if let Some(b) = stack.last_mut() {
                            b.borders = Some(kumihan::TableBorders::nashi());
                        }
                    }
                    b"tcPr" => in_tcpr = true,
                    b"tcBorders" => in_tc_borders = true,
                    // **余白の子は罫線と同じ辺の名前**(`w:top` など)なので、
                    // どの囲みの中に居るかを覚えます(2026-09-03)
                    b"tcMar" => in_tc_mar = true,
                    b"tblCellMar" if in_tblpr => in_tbl_cell_mar = true,
                    // **セルの塗り。** 段落の `w:shd` と名前が同じなので、
                    // `w:tcPr` の中に居るかどうかで見分けます(2026-09-03)
                    b"shd" if in_tcpr => {
                        cell_shade = attr(&e, "fill")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
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
                              align_itta = false;
                              tab_stops.clear();
                              first_line_chars = None;
                              list = ListKind::default(); indent = 0; first_line = 0;
                              line_spacing = 0.0;
                              line_pt = None;
                              space_before_pt = 0.0;
                              space_after_pt = 0.0;
                              page_break_before = std::mem::take(&mut tsugi_kaipeji);
                              shade = None; boxed = false;
                              para_border = kumihan::ParaBorder::default();
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
                    // w:val="0"/"false" は「付けない」の意味なので、有無だけで判定しない。
                    // **要素が在ったこと自体も覚えます**(2026-09-01)。
                    // 「言わない」と「切ると言った」は別で、前者はスタイルから
                    // 受け継ぎ、後者はスタイルを打ち消します
                    b"b" if in_rpr => { fmt.bold = on(&e); fmt.itta.bold = true }
                    b"i" if in_rpr => { fmt.italic = on(&e); fmt.itta.italic = true }
                    b"u" if in_rpr => {
                        fmt.underline = attr(&e, "val").as_deref() != Some("none");
                        fmt.itta.underline = true;
                    }
                    b"strike" if in_rpr => { fmt.strike = on(&e); fmt.itta.strike = true }
                    // **字間**(`w:rPr` の `w:spacing`。1/20 pt)。段落の
                    // `w:spacing`(行の高さ)とは別物なので、`in_rpr` で分けます
                    b"spacing" if in_rpr => {
                        fmt.spacing_pt = attr(&e, "val")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v / 20.0)
                            .unwrap_or(0.0);
                    }
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
                        let n: Option<u32> = attr(&e, "val").and_then(|v| v.parse().ok());
                        // **文書が決めた印を先に引きます**(2026-08-31)。
                        // 無い docx は今までどおり numId の決め打ちです
                        list_text = n.and_then(|n| shirushi.get(&(n, ilvl)).cloned()).map(
                            |(t, kazu)| {
                                list = if kazu { ListKind::Number } else { ListKind::Bullet };
                                t
                            },
                        );
                        if list_text.is_none() {
                            list = match n {
                                Some(2) => ListKind::Number,
                                Some(0) | None => ListKind::None,
                                _ => ListKind::Bullet,
                            };
                        }
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
                        // **段数とは別に、twip をそのまま持ちます**(2026-08-30)。
                        // 段数は 420 twip きざみなので、1文字(210)の字下げが
                        // 2文字になり、3文字が4文字になっていました。内閣府の
                        // 告知書で 123 か所ずれていました。
                        //
                        // `w:leftChars` は文字数(100 = 1文字)での指定で、
                        // 日本語の Word がよく使います。**Word はこちらを
                        // 優先する**ので、両方あればこちらを採ります
                        left_twips = attr(&e, "leftChars")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 100.0 * 210.0) as i32)
                            .or_else(|| {
                                attr(&e, "left").and_then(|v| v.parse::<f32>().ok()).map(|v| v as i32)
                            })
                            .unwrap_or(0);
                        // 1行目の字下げは twip のまま(段落を触っても落とさない —
                        // 2026-08-13 に「黙って消える」を実測で踏んだ)
                        // 文字数の指定(`w:firstLineChars` / `w:hangingChars`)を
                        // 先に見ます。上の `w:leftChars` と同じ理由です
                        let ji = |na: &str| {
                            attr(&e, na)
                                .and_then(|v| v.parse::<f32>().ok())
                                .map(|v| (v / 100.0 * 210.0) as i32)
                        };
                        // 文字数の指定は、そのまま覚えておきます。組むときは
                        // その段落の字の大きさで解き直します
                        first_line_chars = attr(&e, "firstLineChars")
                            .and_then(|v| v.parse::<f32>().ok())
                            .or_else(|| {
                                attr(&e, "hangingChars")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -v)
                            });
                        // **twip は原文のまま持ちます。** Word が書き置いた
                        // `w:firstLine` はその段落の字の大きさで解いた値で、
                        // python-docx が返すのもこれです。組むときは上で
                        // 覚えた文字数から解き直します
                        first_line = attr(&e, "firstLine")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v as i32)
                            .or_else(|| ji("firstLineChars"))
                            .or_else(|| ji("hangingChars").map(|v| -v))
                            .or_else(|| {
                                attr(&e, "hanging")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -(v as i32))
                            })
                            .unwrap_or(0);
                    }
                    b"spacing" if in_ppr => {
                        (line_spacing, line_pt) = gyou_bairitsu(
                            attr(&e, "line").and_then(|v| v.parse::<f32>().ok()),
                            attr(&e, "lineRule"),
                        );
                        // **段落の前後の空き**(twips = pt × 20)。
                        // 前は読んでいなかったので、開いて保存すると消えていた
                        let twips = |n: &str| {
                            attr(&e, n).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 20.0)
                        };
                        space_before_pt = twips("before").unwrap_or(0.0).max(0.0);
                        space_after_pt = twips("after").unwrap_or(0.0).max(0.0);
                    }
                    b"jc" if in_ppr => {
                        align_itta = true;
                        if let Some(v) = attr(&e, "val") { align = Align::from_docx(&v); }
                    }
                    // 表の置き方・スタイル名・列幅の固定(w:tblPr の中)
                    b"jc" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.align = attr(&e, "val").map(|v| Align::from_docx(&v));
                    },
                    b"tblStyle" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.style = attr(&e, "val").filter(|v| !v.is_empty());
                    },
                    // **表スタイルのどの条件を効かせるか**(`w:tblLook`)。
                    // 見出し行や帯を出すかは表ごとに選べます(2026-09-03)
                    b"tblLook" if in_tblpr => if let Some(b) = stack.last_mut() {
                        let on = |k: &str| attr(&e, k).as_deref() == Some("1");
                        b.look = kumihan::TblLook {
                            first_row: on("firstRow"),
                            last_row: on("lastRow"),
                            first_col: on("firstColumn"),
                            last_col: on("lastColumn"),
                            no_h_band: on("noHBand"),
                            no_v_band: on("noVBand"),
                        };
                    },
                    b"tblLayout" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.fixed_layout = attr(&e, "type").as_deref() == Some("fixed");
                    },
                    // **表の幅**(`w:tblW`)。割合(`pct`)のときだけ覚えます。
                    // docx は 1/50 % で書くので 5000 が 100% です。`dxa` は
                    // `w:gridCol` の合計と同じ値なので、読まなくても同じです
                    b"tblW" if in_tblpr => if let Some(b) = stack.last_mut() {
                        if attr(&e, "type").as_deref() == Some("pct") {
                            b.width_pct = attr(&e, "w")
                                .and_then(|v| v.parse::<f32>().ok())
                                .map(|v| v / 50.0)
                                .filter(|v| *v > 0.0);
                        }
                    },
                    // **表の左のインデント**(`w:tblInd`)。原文のまま持ちます
                    b"tblInd" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.ind_twips = attr(&e, "w").and_then(|v| v.parse::<f32>().ok());
                    },
                    // **見出しの行**(`w:tblHeader`)。この行は紙をまたぐたびに
                    // 繰り返します。`w:val="0"` は「繰り返さない」です
                    b"tblHeader" => if stack.last().is_some() {
                        row_header = !matches!(attr(&e, "val").as_deref(), Some("0") | Some("false"));
                    },
                    // **セルの幅いっぱいに字を配る**(`w:tcFitText`)
                    b"tcFitText" if in_tcpr => {
                        cell_fit = !matches!(attr(&e, "val").as_deref(), Some("0") | Some("false"));
                    }
                    // **セルの斜線**(`w:tl2br` は左上から右下、`w:tr2bl` は
                    // 左下から右上)。記入しない欄に引きます
                    b"tl2br" | b"tr2bl" if in_tc_borders => {
                        let hiku = !matches!(attr(&e, "val").as_deref(), Some("none") | Some("nil"));
                        if n.as_slice() == b"tl2br" {
                            cell_borders.diag_down = hiku;
                        } else {
                            cell_borders.diag_up = hiku;
                        }
                    }
                    // 段落の背景色。fill が色(auto 以外)のときだけ
                    b"shd" if in_ppr => {
                        shade = attr(&e, "fill")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // 段落の囲み枠。辺の別は持たない(あれば囲みとみなす)
                    b"pBdr" if in_ppr => { boxed = true; in_pbdr = true }
                    // **どの辺を引くか。** `w:tcBorders`(セルの罫線)にも
                    // 同じ名前の子が並ぶので、`in_pbdr` で見分けます
                    b"top" | b"bottom" | b"left" | b"right" | b"between" if in_pbdr => {
                        if !matches!(attr(&e, "val").as_deref(), Some("none") | Some("nil")) {
                            let b = &mut para_border;
                            // `w:space` は pt そのもの、`w:sz` は 1/8 pt
                            if let Some(v) = attr(&e, "space").and_then(|v| v.parse::<f32>().ok()) {
                                b.space_pt = b.space_pt.max(v);
                            }
                            if let Some(v) = attr(&e, "sz").and_then(|v| v.parse::<f32>().ok()) {
                                b.w_pt = b.w_pt.max(v / 8.0);
                            }
                            match local(e.name().as_ref()) {
                                b"top" => b.top = true,
                                b"bottom" => b.bottom = true,
                                b"left" => b.left = true,
                                b"right" => b.right = true,
                                _ => b.between = true,
                            }
                        }
                    }
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
                    b"vAlign" => if stack.last().is_some() {
                        cell_valign = match attr(&e, "val").as_deref() {
                            Some("center") => book::VAlign::Middle,
                            Some("bottom") => book::VAlign::Bottom,
                            _ => book::VAlign::Top,
                        };
                    },
                    b"trHeight" => if stack.last().is_some() {
                        row_twips = attr(&e, "val").and_then(|v| v.parse().ok());
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
                                // 途中の節の区切り。段落に持たせて保存で返します
                                // (以前はここで doc.sect_raw を上書きしていて、
                                // 区切りごと保存で消えていました)。
                                // 原文・用紙の寸法・改ページするかを一緒に持ちます。
                                // 改ページするかは、ここでは決められません。
                                // docx の `w:type` は「この sectPr で終わる節の
                                // 始め方」なので、この区切りの後で改ページするかは
                                // 1つ後の sectPr を読まないと分かりません。
                                // 本文を読み終えてから [`section_starts`] で埋めます
                                para_sect = Some(kumihan::SectionBreak {
                                    raw: raw.to_string(),
                                    page: parse_sect(raw),
                                    continuous: false,
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
                            // **表示用: OMML を LaTeX に直す。** 原文の控えとは
                            // 別に、組める形にした物を段落に置きます。こちらは
                            // LaTeX を typst で組めるので、これで数式が絵として
                            // 出ます(bytes は空のまま — 組むのは表示する側)
                            if let Some(tex) = kumihan::omml::to_latex(raw) {
                                let off = para.as_ref().map_or(0, |ps: &Vec<Run>| {
                                    ps.iter().map(|r| r.text.len()).sum::<usize>()
                                }) + cur.len();
                                images.push(kumihan::InlineImage {
                                    bytes: std::sync::Arc::new(Vec::new()),
                                    w_mm: 0.0,
                                    h_mm: 0.0,
                                    tex: Some(tex),
                                    src: None,
                                    off,
                                });
                            }
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
                            if let Some(mut im) = image_of(raw, media) {
                                // **字の中のどこに居るか**を覚えます。
                                // 段落の頭に在る絵は行の中に置きます
                                im.off = para.as_ref().map_or(0, |ps: &Vec<Run>| {
                                    ps.iter().map(|r| r.text.len()).sum::<usize>()
                                }) + cur.len();
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
                    // 記入欄(コンテンツコントロール)。外側の要素を読んで、
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
                    // **罫線の辺**。`w:val="nil"` と `"none"` は引かない印
                    // **セルの余白の辺**(`w:tcMar` と `w:tblCellMar` の子)。
                    // 2012 年版の綴りの `w:start` / `w:end` も受けます
                    b"top" | b"left" | b"bottom" | b"right" | b"start" | b"end"
                        if in_tc_mar || in_tbl_cell_mar =>
                    {
                        let mm = attr(&e, "w")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(twip_mm)
                            .unwrap_or(0.0);
                        let at = match n.as_slice() {
                            b"top" => 0,
                            b"right" | b"end" => 1,
                            b"bottom" => 2,
                            _ => 3,
                        };
                        if in_tc_mar {
                            cell_mar.get_or_insert([0.0; 4])[at] = mm;
                        } else if let Some(b) = stack.last_mut() {
                            b.cell_mar_mm.get_or_insert([0.0; 4])[at] = mm;
                        }
                    }
                    b"top" | b"left" | b"bottom" | b"right" | b"insideH" | b"insideV"
                        if in_tbl_borders || in_tc_borders =>
                    {
                        let hiku = !matches!(attr(&e, "val").as_deref(), Some("nil") | Some("none"));
                        if in_tbl_borders {
                            if let Some(b) = stack.last_mut() {
                                let bd = b.borders.get_or_insert_with(kumihan::TableBorders::nashi);
                                match n.as_slice() {
                                    b"top" => bd.top = hiku,
                                    b"left" => bd.left = hiku,
                                    b"bottom" => bd.bottom = hiku,
                                    b"right" => bd.right = hiku,
                                    b"insideH" => bd.inside_h = hiku,
                                    _ => bd.inside_v = hiku,
                                }
                            }
                        } else {
                            match n.as_slice() {
                                b"top" => cell_borders.top = Some(hiku),
                                b"left" => cell_borders.left = Some(hiku),
                                b"bottom" => cell_borders.bottom = Some(hiku),
                                b"right" => cell_borders.right = Some(hiku),
                                _ => {}
                            }
                        }
                    }
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
                    // **`w:type="page"` は改ページ**です(2026-09-01 発注者
                    // 「告知書がおかしいのは7ページ。重複している」)。
                    // 種類を見ずに改行として読んでいたので、内閣府の告知書の
                    // 法令の抄が前の頁に重なって出ていました。
                    //
                    // 段落の途中で割れる形は、次の段落の頭で割ります —
                    // この印は段落の最後の run に置かれるのが普通です
                    b"br" => {
                        if attr(&e, "type").as_deref() == Some("page") {
                            tsugi_kaipeji = true;
                        } else if let Some(p) = para.as_mut() {
                            p.push(Run {
                                text: "\n".into(),
                                size_pt,
                                font: font.clone(),
                                fmt: fmt.clone(),
                            });
                        }
                    }
                    // **タブの文字は run の中の `w:tab` だけです。**
                    // `w:pPr/w:tabs` の中にも同じ名前の要素が並びますが、
                    // あちらはタブを打ったとき字が止まる位置の定義です。
                    // 見分けていなかったので、内閣府の調査票は開いて保存
                    // しただけで行頭にタブが増えていました(2026-09-01)。
                    // タブの止まる位置(`w:pPr/w:tabs` の中の `w:tab w:pos`)
                    b"tab" if in_ppr => {
                        if let Some(v) = attr(&e, "pos").and_then(|v| v.parse::<i32>().ok()) {
                            if v > 0 && !tab_stops.contains(&v) {
                                tab_stops.push(v);
                            }
                        }
                    }
                    b"tab" if !in_ppr => if let Some(p) = para.as_mut() {
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
                    // w:val="0"/"false" は「付けない」の意味なので、有無だけで判定しない。
                    // **要素が在ったこと自体も覚えます**(2026-09-01)。
                    // 「言わない」と「切ると言った」は別で、前者はスタイルから
                    // 受け継ぎ、後者はスタイルを打ち消します
                    b"b" if in_rpr => { fmt.bold = on(&e); fmt.itta.bold = true }
                    b"i" if in_rpr => { fmt.italic = on(&e); fmt.itta.italic = true }
                    b"u" if in_rpr => {
                        fmt.underline = attr(&e, "val").as_deref() != Some("none");
                        fmt.itta.underline = true;
                    }
                    b"strike" if in_rpr => { fmt.strike = on(&e); fmt.itta.strike = true }
                    // **字間**(`w:rPr` の `w:spacing`。1/20 pt)。段落の
                    // `w:spacing`(行の高さ)とは別物なので、`in_rpr` で分けます
                    b"spacing" if in_rpr => {
                        fmt.spacing_pt = attr(&e, "val")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v / 20.0)
                            .unwrap_or(0.0);
                    }
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
                        let n: Option<u32> = attr(&e, "val").and_then(|v| v.parse().ok());
                        // **文書が決めた印を先に引きます**(2026-08-31)。
                        // 無い docx は今までどおり numId の決め打ちです
                        list_text = n.and_then(|n| shirushi.get(&(n, ilvl)).cloned()).map(
                            |(t, kazu)| {
                                list = if kazu { ListKind::Number } else { ListKind::Bullet };
                                t
                            },
                        );
                        if list_text.is_none() {
                            list = match n {
                                Some(2) => ListKind::Number,
                                Some(0) | None => ListKind::None,
                                _ => ListKind::Bullet,
                            };
                        }
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
                        // **段数とは別に、twip をそのまま持ちます**(2026-08-30)。
                        // 段数は 420 twip きざみなので、1文字(210)の字下げが
                        // 2文字になり、3文字が4文字になっていました。内閣府の
                        // 告知書で 123 か所ずれていました。
                        //
                        // `w:leftChars` は文字数(100 = 1文字)での指定で、
                        // 日本語の Word がよく使います。**Word はこちらを
                        // 優先する**ので、両方あればこちらを採ります
                        left_twips = attr(&e, "leftChars")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| (v / 100.0 * 210.0) as i32)
                            .or_else(|| {
                                attr(&e, "left").and_then(|v| v.parse::<f32>().ok()).map(|v| v as i32)
                            })
                            .unwrap_or(0);
                        // 1行目の字下げは twip のまま(段落を触っても落とさない —
                        // 2026-08-13 に「黙って消える」を実測で踏んだ)
                        // 文字数の指定(`w:firstLineChars` / `w:hangingChars`)を
                        // 先に見ます。上の `w:leftChars` と同じ理由です
                        let ji = |na: &str| {
                            attr(&e, na)
                                .and_then(|v| v.parse::<f32>().ok())
                                .map(|v| (v / 100.0 * 210.0) as i32)
                        };
                        // 文字数の指定は、そのまま覚えておきます。組むときは
                        // その段落の字の大きさで解き直します
                        first_line_chars = attr(&e, "firstLineChars")
                            .and_then(|v| v.parse::<f32>().ok())
                            .or_else(|| {
                                attr(&e, "hangingChars")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -v)
                            });
                        // **twip は原文のまま持ちます。** Word が書き置いた
                        // `w:firstLine` はその段落の字の大きさで解いた値で、
                        // python-docx が返すのもこれです。組むときは上で
                        // 覚えた文字数から解き直します
                        first_line = attr(&e, "firstLine")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v as i32)
                            .or_else(|| ji("firstLineChars"))
                            .or_else(|| ji("hangingChars").map(|v| -v))
                            .or_else(|| {
                                attr(&e, "hanging")
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|v| -(v as i32))
                            })
                            .unwrap_or(0);
                    }
                    b"spacing" if in_ppr => {
                        (line_spacing, line_pt) = gyou_bairitsu(
                            attr(&e, "line").and_then(|v| v.parse::<f32>().ok()),
                            attr(&e, "lineRule"),
                        );
                        // **こちらは空要素(`<w:spacing …/>`)の腕。**
                        // 実際に書かれるのはほぼこちらなので、上の Start の腕
                        // だけ直しても効かない(2026-08-15 に踏んだ)
                        let twips = |n: &str| {
                            attr(&e, n).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 20.0)
                        };
                        space_before_pt = twips("before").unwrap_or(0.0).max(0.0);
                        space_after_pt = twips("after").unwrap_or(0.0).max(0.0);
                    }
                    b"jc" if in_ppr => {
                        align_itta = true;
                        if let Some(v) = attr(&e, "val") { align = Align::from_docx(&v); }
                    }
                    // 表の置き方・スタイル名・列幅の固定(w:tblPr の中)
                    b"jc" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.align = attr(&e, "val").map(|v| Align::from_docx(&v));
                    },
                    b"tblStyle" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.style = attr(&e, "val").filter(|v| !v.is_empty());
                    },
                    // **表スタイルのどの条件を効かせるか**(`w:tblLook`)。
                    // 見出し行や帯を出すかは表ごとに選べます(2026-09-03)
                    b"tblLook" if in_tblpr => if let Some(b) = stack.last_mut() {
                        let on = |k: &str| attr(&e, k).as_deref() == Some("1");
                        b.look = kumihan::TblLook {
                            first_row: on("firstRow"),
                            last_row: on("lastRow"),
                            first_col: on("firstColumn"),
                            last_col: on("lastColumn"),
                            no_h_band: on("noHBand"),
                            no_v_band: on("noVBand"),
                        };
                    },
                    b"tblLayout" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.fixed_layout = attr(&e, "type").as_deref() == Some("fixed");
                    },
                    // **表の幅**(`w:tblW`)。割合(`pct`)のときだけ覚えます。
                    // docx は 1/50 % で書くので 5000 が 100% です。`dxa` は
                    // `w:gridCol` の合計と同じ値なので、読まなくても同じです
                    b"tblW" if in_tblpr => if let Some(b) = stack.last_mut() {
                        if attr(&e, "type").as_deref() == Some("pct") {
                            b.width_pct = attr(&e, "w")
                                .and_then(|v| v.parse::<f32>().ok())
                                .map(|v| v / 50.0)
                                .filter(|v| *v > 0.0);
                        }
                    },
                    // **表の左のインデント**(`w:tblInd`)。原文のまま持ちます
                    b"tblInd" if in_tblpr => if let Some(b) = stack.last_mut() {
                        b.ind_twips = attr(&e, "w").and_then(|v| v.parse::<f32>().ok());
                    },
                    // **見出しの行**(`w:tblHeader`)。この行は紙をまたぐたびに
                    // 繰り返します。`w:val="0"` は「繰り返さない」です
                    b"tblHeader" => if stack.last().is_some() {
                        row_header = !matches!(attr(&e, "val").as_deref(), Some("0") | Some("false"));
                    },
                    // **セルの幅いっぱいに字を配る**(`w:tcFitText`)
                    b"tcFitText" if in_tcpr => {
                        cell_fit = !matches!(attr(&e, "val").as_deref(), Some("0") | Some("false"));
                    }
                    // **セルの斜線**(`w:tl2br` は左上から右下、`w:tr2bl` は
                    // 左下から右上)。記入しない欄に引きます
                    b"tl2br" | b"tr2bl" if in_tc_borders => {
                        let hiku = !matches!(attr(&e, "val").as_deref(), Some("none") | Some("nil"));
                        if n.as_slice() == b"tl2br" {
                            cell_borders.diag_down = hiku;
                        } else {
                            cell_borders.diag_up = hiku;
                        }
                    }
                    // 段落の背景色。fill が色(auto 以外)のときだけ
                    b"shd" if in_ppr => {
                        shade = attr(&e, "fill")
                            .filter(|v| !v.is_empty() && v != "auto");
                    }
                    // 段落の囲み枠。辺の別は持たない(あれば囲みとみなす)
                    b"pBdr" if in_ppr => { boxed = true; in_pbdr = true }
                    // **どの辺を引くか。** `w:tcBorders`(セルの罫線)にも
                    // 同じ名前の子が並ぶので、`in_pbdr` で見分けます
                    b"top" | b"bottom" | b"left" | b"right" | b"between" if in_pbdr => {
                        if !matches!(attr(&e, "val").as_deref(), Some("none") | Some("nil")) {
                            let b = &mut para_border;
                            // `w:space` は pt そのもの、`w:sz` は 1/8 pt
                            if let Some(v) = attr(&e, "space").and_then(|v| v.parse::<f32>().ok()) {
                                b.space_pt = b.space_pt.max(v);
                            }
                            if let Some(v) = attr(&e, "sz").and_then(|v| v.parse::<f32>().ok()) {
                                b.w_pt = b.w_pt.max(v / 8.0);
                            }
                            match local(e.name().as_ref()) {
                                b"top" => b.top = true,
                                b"bottom" => b.bottom = true,
                                b"left" => b.left = true,
                                b"right" => b.right = true,
                                _ => b.between = true,
                            }
                        }
                    }
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
                    b"vAlign" => if stack.last().is_some() {
                        cell_valign = match attr(&e, "val").as_deref() {
                            Some("center") => book::VAlign::Middle,
                            Some("bottom") => book::VAlign::Bottom,
                            _ => book::VAlign::Top,
                        };
                    },
                    b"trHeight" => if stack.last().is_some() {
                        row_twips = attr(&e, "val").and_then(|v| v.parse().ok());
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
                    b"pBdr" => in_pbdr = false,
                    b"tblPr" => in_tblpr = false,
                    b"p" => {
                        if let Some(runs) = para.take() {
                            rep.runs += runs.len();
                            rep.paragraphs += 1;
                            let mut p = Paragraph { align, raw_adoc: None, list_text: list_text.take(),
                                            anchors: std::mem::take(&mut anchors),
                                sect: para_sect.take(),
                                images: std::mem::take(&mut images),
                                comments: std::mem::take(&mut para_comments),
                                bookmarks: std::mem::take(&mut para_bookmarks),
                                page_break_before, list,
                                // 深さ: w:ind(直接指定)が無ければ w:ilvl から
                                indent: indent.max(ilvl),
                                left_twips,
                                first_line_twips: first_line,
                                first_line_chars,
                                align_itta,
                                tab_stops: std::mem::take(&mut tab_stops),
                                line_spacing,
                                line_pt,
                                space_before_pt,
                                space_after_pt,
                                style: pstyle,
                                style_id: pstyle_id.take(),
                                shade: shade.take(), boxed,
                                border: para_border,
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
                            borders: cell_borders,
                            col_span: cell_span,
                            v_merge: cell_vmerge,
                            valign: cell_valign,
                            shade: cell_shade.take(),
                            mar_mm: cell_mar.take(),
                            fit_text: cell_fit,
                        });
                        cell_span = 0;
                        cell_vmerge = VMerge::None;
                        cell_valign = book::VAlign::Top;
                        cell_borders = kumihan::CellBorders::default();
                        cell_fit = false;
                    },
                    b"tblBorders" => in_tbl_borders = false,
                    b"tcPr" => in_tcpr = false,
                    b"tcBorders" => in_tc_borders = false,
                    b"tcMar" => in_tc_mar = false,
                    b"tblCellMar" => in_tbl_cell_mar = false,
                    b"tr" => if let Some(b) = stack.last_mut() {
                        // **見出しの行は最初の行だけ持ちます**(2026-09-03)。
                        // 模型は「見出しの行があるか」の1つしか持たないので、
                        // 2行目以降に付いた `w:tblHeader` は落とします
                        if row_header && b.rows.is_empty() {
                            b.header_row = true;
                        }
                        row_header = false;
                        let row = std::mem::take(&mut b.row);
                        b.rows.push(row);
                        // 指定の無い行は 0(= 中身なり)。**行と同じ長さで
                        // 持つ**ので、後ろの行だけ高さが付いていても
                        // 添字がずれません
                        b.row_mm.push(row_twips.take().map_or(0.0, |t| twip_mm(t as f32)));
                    },
                    b"tbl" => {
                        if let Some(b) = stack.pop() {
                            let tb = Table {
                                rows: b.rows,
                                // **docx の既定は「引かない」です。** 表も
                                // スタイルも何も言っていない表に、Word は
                                // 罫線を引きません。四方に引くのは AsciiDoc
                                // の表の決めで、docx には合いません
                                // (2026-08-30)。スタイルが言っていれば
                                // 後で [`hyou_no_kei`] が上書きします
                                borders: b.borders.unwrap_or_else(kumihan::TableBorders::nashi),
                                style_borders_unset: b.borders.is_none(),
                                col_mm: b.col_mm,
                                // どの行にも指定が無ければ、持たないのと同じ
                                row_mm: if b.row_mm.iter().all(|h| *h <= 0.0) {
                                    Vec::new()
                                } else {
                                    b.row_mm
                                },
                                // 役割は `.sheet.adoc` の印なので docx には無い
                                role: None,
                                // docx は幅を mm で持つので、割合は空のまま
                                col_ratio: Vec::new(),
                                header_row: b.header_row,
                                // docx の表は題を持たない
                                title: None,
                                style: b.style,
                                look: b.look,
                                cell_mar_mm: b.cell_mar_mm,
                                align: b.align,
                                fixed_layout: b.fixed_layout,
                                width_pct: b.width_pct,
                                // **`w:tblInd` はここでは原文のまま**(mm に
                                // 直すだけ)です。セルの余白を引く補正は
                                // 設定を読める [`tblind_wo_naosu`] でします
                                indent_mm: b.ind_twips.map(twip_mm).unwrap_or(0.0),
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
    section_starts(&mut doc);
    (doc, rep)
}

/// 節の始め方を、段落が持つ区切りに写します。
///
/// docx の `w:type`(nextPage・continuous など)は、その sectPr で終わる
/// 節の始め方です。Word も python-docx もそう読みます。
/// 模型の [`kumihan::SectionBreak::continuous`] は「この区切りで改ページ
/// しないか」、つまり次の節の始め方です。
/// そのため、区切りの値は1つ後の sectPr(次の区切りか、文書末の sectPr)の
/// `w:type` から取ります。
/// 前は区切り自身の `w:type` を読んでいました。Word で continuous にした節が、
/// officework では1つ前の節にずれて見えていました(2026-09-02)。
///
/// 表のセルの中の段落は見ません。Word は表の中に節の区切りを置きません。
pub(super) fn section_starts(doc: &mut Document) {
    let idx: Vec<usize> = doc
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| matches!(b, Block::Para(p) if p.sect.is_some()))
        .map(|(i, _)| i)
        .collect();
    for (k, &i) in idx.iter().enumerate() {
        let next_raw: Option<String> = match idx.get(k + 1) {
            Some(&j) => match &doc.blocks[j] {
                Block::Para(p) => p.sect.as_ref().map(|s| s.raw.clone()),
                Block::Table(_) => None,
            },
            None => doc.sect_raw.clone(),
        };
        let cont = next_raw.as_deref().is_some_and(|r| sect_type(r) == "continuous");
        if let Block::Para(p) = &mut doc.blocks[i] {
            if let Some(s) = p.sect.as_mut() {
                s.continuous = cont;
            }
        }
    }
}
/// **ページに貼り付く図形を DrawingML にする。**
///
/// 2026-08-29 発注者「docx の図形をやって」。ペンの筆
/// ([`ink_anchor_xml`])と同じ置き方(`wp:anchor` の `relativeFrom="page"`)
/// で、形は xlsx と同じ prstGeom / custGeom です。**Word でも図形として
/// 開けます** — 絵に落とさないのが決めです。
///
/// 名前 `joshape…p{page}` が読み戻しの鍵です(ページ番号も名前で持ちます —
/// XML の座標はページの中の位置しか持てないため)。
pub fn shape_anchor_xml(sp: &kumihan::DocShape, id: usize) -> String {
    let (w, h) = (sp.w_mm.max(0.5), sp.h_mm.max(0.5));
    let (cx, cy) = (emu(w), emu(h));
    let look = &sp.look;

    // 形。点を持つ物は custGeom、それ以外は prstGeom の名前をそのまま
    let geom = if look.points.is_empty() {
        let prst = match look.kind.as_str() {
            "roundRect" | "ellipse" | "rightArrow" | "diamond" | "line" => look.kind.as_str(),
            _ => "rect",
        };
        format!(r#"<a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>"#)
    } else {
        let mut path = String::new();
        for (i, p) in look.points.iter().enumerate() {
            let tag = if i == 0 || p.start { "moveTo" } else { "lnTo" };
            path.push_str(&format!(
                r#"<a:{tag}><a:pt x="{}" y="{}"/></a:{tag}>"#,
                (p.at.0.clamp(0.0, 1.0) * cx as f32) as i64,
                (p.at.1.clamp(0.0, 1.0) * cy as f32) as i64,
            ));
        }
        format!(
            concat!(
                r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>"#,
                r#"<a:rect l="0" t="0" r="{cx}" b="{cy}"/>"#,
                r#"<a:pathLst><a:path w="{cx}" h="{cy}">{path}</a:path></a:pathLst></a:custGeom>"#
            ),
            cx = cx,
            cy = cy,
            path = path
        )
    };

    // 塗りと線。**指定が無ければ書きません**(模型の決め — 無い物を
    // 黒や白に落とすと、字を置くだけの箱にも枠が出ます)
    let a = look.alpha.clamp(0.0, 1.0);
    let usu = if a >= 1.0 {
        String::new()
    } else {
        format!(r#"<a:alpha val="{}"/>"#, (a * 100_000.0) as i64)
    };
    let fill = match look.fill.as_deref() {
        Some(c) => format!(r#"<a:solidFill><a:srgbClr val="{}">{usu}</a:srgbClr></a:solidFill>"#,
                           c.trim_start_matches('#')),
        None => "<a:noFill/>".into(),
    };
    let line = match look.line.as_deref() {
        Some(c) => format!(
            r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}">{usu}</a:srgbClr></a:solidFill></a:ln>"#,
            (look.line_w.max(0.1) * 12_700.0) as i64,
            c.trim_start_matches('#')
        ),
        None => String::new(),
    };
    // 影。xlsx と同じ右下への落ち影です
    let kage = if look.shadow {
        concat!(
            r#"<a:effectLst><a:outerShdw blurRad="38100" dist="38100" dir="2700000" "#,
            r#"algn="tl" rotWithShape="0"><a:srgbClr val="9E9E9E">"#,
            r#"<a:alpha val="35000"/></a:srgbClr></a:outerShdw></a:effectLst>"#
        )
        .to_string()
    } else {
        String::new()
    };
    // 回転(6万分の1度)と反転
    let rot = look.rot.rem_euclid(360.0);
    let mut xfrm = String::new();
    if rot != 0.0 {
        xfrm.push_str(&format!(r#" rot="{}""#, (rot * 60_000.0) as i64));
    }
    if look.flip_h {
        xfrm.push_str(r#" flipH="1""#);
    }
    if look.flip_v {
        xfrm.push_str(r#" flipV="1""#);
    }
    // 図形の中の文字
    let body = match look.text.as_deref() {
        Some(t) if !t.is_empty() => format!(
            r#"<wps:txbx><w:txbxContent><w:p><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p></w:txbxContent></wps:txbx>"#,
            esc(t)
        ),
        _ => String::new(),
    };

    format!(
        concat!(
            r#"<w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" "#,
            r#"relativeHeight="251658241" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">"#,
            r#"<wp:simplePos x="0" y="0"/>"#,
            r#"<wp:positionH relativeFrom="page"><wp:posOffset>{px}</wp:posOffset></wp:positionH>"#,
            r#"<wp:positionV relativeFrom="page"><wp:posOffset>{py}</wp:posOffset></wp:positionV>"#,
            r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:wrapNone/>"#,
            r#"<wp:docPr id="{id}" name="joshape{id}p{page}"/>"#,
            r#"<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"<wps:wsp><wps:cNvSpPr/><wps:spPr>"#,
            r#"<a:xfrm{xfrm}><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"{geom}{fill}{line}{kage}"#,
            r#"</wps:spPr>{body}<wps:bodyPr rot="0" anchor="ctr"/></wps:wsp>"#,
            r#"</a:graphicData></a:graphic></wp:anchor></w:drawing>"#
        ),
        px = emu(sp.x_mm),
        py = emu(sp.y_mm),
        cx = cx,
        cy = cy,
        id = id,
        page = sp.page,
        xfrm = xfrm,
        geom = geom,
        fill = fill,
        line = line,
        kage = kage,
        body = body
    )
}

/// [`shape_anchor_xml`] を、段落の控え(anchors)にそのまま置ける形で返す
pub fn shape_anchor_run(sp: &kumihan::DocShape, id: usize) -> String {
    let inner = shape_anchor_xml(sp, id);
    wrap_with_ns(&inner, &Default::default()).unwrap_or(inner)
}

/// **`w:tblInd` の測り方は Word の版で違う。**
///
/// Word 2013(`compatibilityMode` が 15 以上)からは、`w:tblInd` は表の
/// 左の罫線の位置です。それより前の書き方では**セルの中の字の位置**を
/// 指すので、セルの左余白のぶんだけ表は左へ戻ります。LibreOffice も
/// `DomainMapperTableHandler` で同じ分け方をしています。
///
/// 設定に `compatibilityMode` が無い docx は古い方の扱いです
/// (LibreOffice の `GetWordCompatibilityMode` は無ければ -1 を返します)。
/// 内閣府の調査票がこれで、`w:tblInd` の 108twip はセルの左余白の
/// 108twip とちょうど打ち消し合い、表は本文の左端から始まります。
fn tblind_wo_naosu(doc: &mut Document, sxml: &str) {
    let mode = sxml
        .find(r#"w:name="compatibilityMode""#)
        .and_then(|k| {
            let seg = &sxml[k..];
            let e = seg.find("/>").unwrap_or(seg.len());
            let tag = &seg[..e];
            let k2 = tag.find("w:val=\"")? + 7;
            let e2 = tag[k2..].find('"')? + k2;
            tag[k2..e2].parse::<i32>().ok()
        })
        .unwrap_or(-1);
    if mode >= 15 {
        return;
    }
    // Word の既定のセルの左余白は 108twip(1.9mm)です
    const KITEI: f32 = 108.0 * 25.4 / 1440.0;
    for b in &mut doc.blocks {
        let kumihan::Block::Table(t) = b else { continue };
        if t.indent_mm == 0.0 {
            continue;
        }
        let hidari = t
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|c| c.mar_mm)
            .map(|m| m[3])
            .or_else(|| t.cell_mar_mm.map(|m| m[3]))
            .unwrap_or(KITEI);
        t.indent_mm -= hidari;
    }
}

/// **表の罫線を、名乗っているスタイルから補う。**
///
/// 表が `w:tblBorders` を持っていなければ、`w:tblStyle` が指す定義の
/// `w:tblBorders` に従います。スタイルにも無ければ、いままでどおり
/// 四方に引きます。
///
/// 2026-08-30。内閣府の様式(document_4.docx)は4つの表のうち3つが
/// スタイル任せで、そのスタイルが「下と横内線だけ」でした。読まないと
/// 枠だらけになります。
fn hyou_no_kei(doc: &mut Document, styxml: &str) {
    fn kei_of(styxml: &str, id: &str) -> Option<kumihan::TableBorders> {
        let i = styxml.find(&format!("w:styleId=\"{id}\""))?;
        let j = styxml[i..].find("</w:style>").map(|e| i + e)?;
        let seg = &styxml[i..j];
        let bi = seg.find("<w:tblBorders>")?;
        let be = seg[bi..].find("</w:tblBorders>").map(|e| bi + e)?;
        let naka = &seg[bi..be];
        let mut bd = kumihan::TableBorders::nashi();
        for (tag, at) in [
            ("<w:top", &mut bd.top), ("<w:left", &mut bd.left),
            ("<w:bottom", &mut bd.bottom), ("<w:right", &mut bd.right),
            ("<w:insideH", &mut bd.inside_h), ("<w:insideV", &mut bd.inside_v),
        ] {
            if let Some(k) = naka.find(tag) {
                let owari = naka[k..].find('>').map(|e| k + e).unwrap_or(naka.len());
                *at = !naka[k..owari].contains("w:val=\"nil\"")
                    && !naka[k..owari].contains("w:val=\"none\"");
            }
        }
        Some(bd)
    }
    fn walk(blocks: &mut [Block], styxml: &str) {
        for b in blocks {
            let Block::Table(t) = b else { continue };
            if !t.style_borders_unset {
                continue;
            }
            if let Some(bd) = t.style.as_deref().and_then(|id| kei_of(styxml, id)) {
                t.borders = bd;
            }
        }
    }
    walk(&mut doc.blocks, styxml);
}

/// 原文控え(anchors)の中の joshape(ページに貼り付く図形)を模型へ読み戻す。
/// 読めたら控えから外す(保存はモデルから作り直すので、二重になりません)。
///
/// **こちらが書いた図形だけを読みます。** Word や他のソフトが作った
/// `wps:wsp` は名前が違うので手を付けず、原本のまま持ち越します —
/// 読めない物を半端に読むより、そのまま返す方が壊しません。
pub(super) fn extract_shapes(doc: &mut Document) {
    let mut deta: Vec<kumihan::DocShape> = Vec::new();
    for b in &mut doc.blocks {
        let Block::Para(p) = b else { continue };
        let mut i = 0;
        while i < p.anchors.len() {
            let a = &p.anchors[i];
            let Some(ni) = a.find("name=\"joshape") else {
                i += 1;
                continue;
            };
            let page = a[ni..]
                .find('p')
                .and_then(|pi| {
                    let s2 = &a[ni + pi + 1..];
                    let e = s2.find('"')?;
                    s2[..e].parse::<usize>().ok()
                })
                .unwrap_or(0);
            let (Some(sp), Some((w, h))) = (shape_look(a, &[]), shape_size(a)) else {
                i += 1;
                continue;
            };
            let x = mm_of(a, "<wp:positionH relativeFrom=\"page\"><wp:posOffset>");
            let y = mm_of(a, "<wp:positionV relativeFrom=\"page\"><wp:posOffset>");
            let (Some(x_mm), Some(y_mm)) = (x, y) else {
                i += 1;
                continue;
            };
            deta.push(kumihan::DocShape { page, x_mm, y_mm, w_mm: w, h_mm: h, look: sp });
            p.anchors.remove(i);
        }
    }
    doc.shapes.extend(deta);
}

/// **他所のソフトが作った図形**(テキストボックスなど)を1つ読みます。
///
/// うちが書く図形は `wp:docPr` の名前が `joshape…` で、ページ番号も名前に
/// 入れてあるので [`extract_shapes`] が読めます。Word が作る図形は名前が
/// 違い、位置も「この段落から下へ○mm」のような**相対**で書いてあるので、
/// 置き場所は組み上がりを知っている側(`paper`)が決めます。ここは
/// 寸法と見た目と**基準の名前**だけ返します。
///
/// 2026-08-30 に足しました。内閣府の告知書の窓口の欄が3つとも消えていて、
/// 紙にも画面にも出ませんでした(保存では原文のまま残っていました)。
#[derive(Debug, Clone)]
pub struct ForeignShape {
    /// 基準からのずれ(mm)
    pub x_mm: f32,
    pub y_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
    /// 横の基準。`margin` / `column` / `page` / `character` など
    pub h_from: String,
    /// 縦の基準。`paragraph` / `page` / `line` など
    pub v_from: String,
    /// **横の寄せ方**(docx の `<wp:align>`)。距離ではなく、基準の中で
    /// どちらへ寄せるかです。`left` / `right` / `center` / `inside` /
    /// `outside`。`posOffset` の代わりに来ます
    pub h_align: Option<String>,
    /// **縦の寄せ方**。`top` / `bottom` / `center` / `inside` / `outside`
    pub v_align: Option<String>,
    /// **幅が何かの百分率のとき**(`wp14:sizeRelH`)。(基準, 割合)。
    /// 基準は `page` / `margin` / `leftMargin` など
    pub w_pct: Option<(String, f32)>,
    /// **高さが何かの百分率のとき**(`wp14:sizeRelV`)
    pub h_pct: Option<(String, f32)>,
    /// 形・塗り・線・中の文字
    pub look: book::SheetShape,
}

/// 段落の控え1つを [`ForeignShape`] に。うちの図形と、図形でない物は `None`
/// **記号の書体の私用領域を、見えている字に直す。**
///
/// Word の既定の箇条書きは、印を書体と組で書きます。Symbol の U+F0B7 は
/// 中黒(•)、Wingdings の U+F0A7 は小さい四角(▪)、Courier New の `o` は
/// そのままです。書体を持たない所で出すには、字の側を直すしかありません。
///
/// 表に無い組は、私用領域の下位バイトが普通の字ならそれを使い、
/// そうでなければ中黒にします(何も出ないよりは印がある方がよい)。
fn kigou_wo_naosu(txt: &str, shotai: &str) -> String {
    let watashi = |c: char| ('\u{f000}'..='\u{f0ff}').contains(&c);
    if !txt.chars().any(watashi) {
        return txt.to_string();
    }
    let s = shotai.to_ascii_lowercase();
    txt.chars()
        .map(|c| {
            if !watashi(c) {
                return c;
            }
            let shita = (c as u32 - 0xf000) as u8;
            match (s.as_str(), shita) {
                ("symbol", 0xb7) => '\u{2022}',       // •
                ("symbol", 0xa7) => '\u{25aa}',       // ▪
                ("wingdings", 0xa7) => '\u{25aa}',    // ▪
                ("wingdings", 0xfc) => '\u{2714}',    // ✔
                ("wingdings", 0x76) => '\u{2612}',    // ☒
                ("wingdings", 0x6f) => '\u{25a1}',    // □
                _ if shita.is_ascii_graphic() => shita as char,
                _ => '\u{2022}',
            }
        })
        .collect()
}

/// **箇条書きの印の表**(docx の `numbering.xml`)。
///
/// 返るのは `(numId, 段) → (印, 番号か)` です。印は `w:lvlText` の字で、
/// `○` や `(%1)` のような書き方をしています。`%1` はその段の番号で、
/// 置き替えるのは組む所です([`kumihan::Paragraph::marker`])。
///
/// docx は2段構えで、`w:num` が `numId` を `abstractNumId` に結び付け、
/// 本体の `w:abstractNum` が段ごとの印を持ちます。**両方引かないと**
/// 印にたどり着けません。
pub(crate) fn num_markers(xml: &str) -> std::collections::BTreeMap<(u32, u8), (String, bool)> {
    let mut out = std::collections::BTreeMap::new();
    if xml.is_empty() {
        return out;
    }
    let attr1 = |seg: &str, key: &str| -> Option<String> {
        let pat = format!("{key}=\"");
        let i = seg.find(&pat)? + pat.len();
        seg[i..].find('"').map(|e| seg[i..i + e].to_string())
    };
    // 本体: abstractNumId → 段ごとの (印, 番号か)
    let mut honnin: std::collections::BTreeMap<u32, Vec<(u8, String, bool)>> = Default::default();
    let mut rest = xml;
    while let Some(i) = rest.find("<w:abstractNum ") {
        let owari = rest[i..].find("</w:abstractNum>").map(|e| i + e).unwrap_or(rest.len());
        let blk = &rest[i..owari];
        let id: Option<u32> = attr1(blk, "w:abstractNumId").and_then(|v| v.parse().ok());
        if let Some(id) = id {
            let mut lv = blk;
            while let Some(j) = lv.find("<w:lvl ") {
                let le = lv[j..].find("</w:lvl>").map(|e| j + e).unwrap_or(lv.len());
                let one = &lv[j..le];
                let ilvl: u8 = attr1(one, "w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                let fmt = one
                    .find("<w:numFmt ")
                    .and_then(|k| attr1(&one[k..], "w:val"))
                    .unwrap_or_default();
                let txt = one
                    .find("<w:lvlText ")
                    .and_then(|k| attr1(&one[k..], "w:val"))
                    .unwrap_or_default();
                // **記号の書体の私用領域を、普通の字に直します。**
                //
                // Word の既定の箇条書きは、印を Symbol 書体の U+F0B7 と
                // 書きます。LibreOffice はその書体ごと持つので出せますが
                // (`NumberingManager.cxx` が `CharFontName` を残します)、
                // こちらは文書の書体1本で組むので、字が無くて何も出ません。
                // 見えている物と同じ字に置き替えます(2026-09-03)
                let shotai = one
                    .find("<w:rFonts ")
                    .and_then(|k| one[k..].find('>').map(|e| &one[k..k + e]))
                    .map(|seg| attr1(seg, "w:ascii").unwrap_or_default())
                    .unwrap_or_default();
                let txt = kigou_wo_naosu(&txt, &shotai);
                if !txt.is_empty() && fmt != "none" {
                    honnin.entry(id).or_default().push((ilvl, txt, fmt != "bullet"));
                }
                lv = &lv[le.max(j + 7)..];
            }
        }
        rest = &rest[owari.max(i + 15)..];
    }
    // 結び付け: numId → abstractNumId
    let mut rest = xml;
    while let Some(i) = rest.find("<w:num ") {
        let owari = rest[i..].find("</w:num>").map(|e| i + e).unwrap_or(rest.len());
        let blk = &rest[i..owari];
        let num: Option<u32> = attr1(blk, "w:numId").and_then(|v| v.parse().ok());
        let abs: Option<u32> = blk
            .find("<w:abstractNumId ")
            .and_then(|k| attr1(&blk[k..], "w:val"))
            .and_then(|v| v.parse().ok());
        if let (Some(n), Some(a)) = (num, abs) {
            for (ilvl, txt, kazu) in honnin.get(&a).into_iter().flatten() {
                out.insert((n, *ilvl), (txt.clone(), *kazu));
            }
        }
        rest = &rest[owari.max(i + 7)..];
    }
    out
}

pub fn foreign_shape(a: &str) -> Option<ForeignShape> {
    foreign_shape_with(a, &[])
}

/// **文書のテーマの配色つき。** 図形の色はテーマの名前で書いてあることが
/// 多いので、これを渡さないと既定の配色で出ます([`crate::theme::dml_iro`])
pub fn foreign_shape_with(a: &str, palette: &[String]) -> Option<ForeignShape> {
    if a.contains("name=\"joshape") || a.contains("name=\"joink") {
        return None; // うちが書いた物は extract_shapes が読みます
    }
    let (w_mm, h_mm) = shape_size(a)?;
    let look = shape_look(a, palette)?;
    // 基準の名前と、そこからのずれ
    //
    // **`<wp:posOffset>` と `<wp:align>` は二択です。** 前者は基準からの
    // 距離、後者は基準の中での寄せ方です。`align` を読まないと、寄せて
    // 置いた図形が全部基準の原点(紙なら左上)へ落ちます。内閣府の
    // 面談の記録の飾り枠がそれでした(2026-09-03 発注者)
    let kijun = |tag: &str| -> (String, f32, Option<String>) {
        let Some(i) = a.find(tag) else { return (String::new(), 0.0, None) };
        let from = a[i..]
            .find("relativeFrom=\"")
            .and_then(|j| {
                let s2 = i + j + 14;
                a[s2..].find('"').map(|e| a[s2..s2 + e].to_string())
            })
            .unwrap_or_default();
        let owari = a[i..].find('>').map(|e| i + e).unwrap_or(a.len());
        // この位置の指定が終わる所まで(次の `</wp:positionH>` など)
        let tojime = format!("</{}>", &tag[1..]);
        let sue = a[owari..]
            .find(&tojime)
            .map(|e| owari + e)
            .unwrap_or(a.len());
        let naka = &a[owari..sue];
        let zure = mm_of(naka, "<wp:posOffset>").unwrap_or(0.0);
        let yose = naka.find("<wp:align>").and_then(|j| {
            let s2 = j + 10;
            naka[s2..].find('<').map(|e| naka[s2..s2 + e].trim().to_string())
        });
        (from, zure, yose)
    };
    let (h_from, x_mm, h_align) = kijun("<wp:positionH");
    let (v_from, y_mm, v_align) = kijun("<wp:positionV");
    // **大きさが紙や余白に対する百分率のことがあります**(Word 2010 の
    // `wp14:sizeRelH` / `wp14:sizeRelV`)。`wp:extent` はそのときの控えで、
    // 実際の大きさはこちらです。内閣府の面談の記録の飾り枠は紙の
    // 92% × 94% で、`wp:extent` の 197.9×261.4mm ではなく
    // 193.2×279.2mm で出ます(2026-09-03)
    let pct = |tag: &str, key: &str| -> Option<(String, f32)> {
        let i = a.find(tag)?;
        let owari = a[i..].find('>').map(|e| i + e).unwrap_or(a.len());
        let from = a[i..owari]
            .find("relativeFrom=\"")
            .and_then(|j| {
                let s2 = i + j + 14;
                a[s2..].find('"').map(|e| a[s2..s2 + e].to_string())
            })?;
        let k = a[owari..].find(key)? + owari + key.len();
        let e = a[k..].find('<')? + k;
        // 1000 分の1パーセント
        a[k..e].trim().parse::<f32>().ok().map(|v| (from, v / 100000.0))
    };
    let w_pct = pct("<wp14:sizeRelH", "<wp14:pctWidth>");
    let h_pct = pct("<wp14:sizeRelV", "<wp14:pctHeight>");
    Some(ForeignShape {
        x_mm, y_mm, w_mm, h_mm, h_from, v_from, h_align, v_align, w_pct, h_pct, look,
    })
}

/// `<wp:extent cx="…" cy="…"/>` を mm で
fn shape_size(a: &str) -> Option<(f32, f32)> {
    let i = a.find("<wp:extent cx=\"")? + 15;
    let xe = a[i..].find('"')? + i;
    let cx: f32 = a[i..xe].parse().ok()?;
    let j = a[xe..].find("cy=\"")? + xe + 4;
    let ye = a[j..].find('"')? + j;
    let cy: f32 = a[j..ye].parse().ok()?;
    Some((cx / 36000.0, cy / 36000.0))
}

/// EMU の値を mm で拾う
fn mm_of(a: &str, pat: &str) -> Option<f32> {
    let i = a.find(pat)? + pat.len();
    let e = a[i..].find('<')? + i;
    a[i..e].parse::<f32>().ok().map(|v| v / 36000.0)
}

/// 図形の見た目(形・塗り・線・回転・不透明度・影・中の文字)
fn shape_look(a: &str, palette: &[String]) -> Option<book::SheetShape> {
    let mut sp = book::SheetShape { alpha: 1.0, line_w: 1.5, ..Default::default() };
    // 形。prstGeom の名前、無ければ点で作る形
    sp.kind = match a.find("<a:prstGeom prst=\"") {
        Some(i) => {
            let s2 = i + 18;
            let e = a[s2..].find('"')? + s2;
            a[s2..e].to_string()
        }
        None if a.contains("<a:custGeom") => "path".into(),
        None => return None,
    };
    // 調整値(prstGeom の avLst)。大かっこの曲がりなど、形のつまみ。
    // custGeom の gdLst と混ざらないよう、avLst の中だけを見る
    if sp.kind != "path" {
        if let Some(i) = a.find("<a:avLst>") {
            let end = a[i..].find("</a:avLst>").map(|e| i + e).unwrap_or(a.len());
            let mut at = i;
            while let Some(j) = a[at..end].find("<a:gd name=\"") {
                let s2 = at + j + 12;
                let Some(ne) = a[s2..end].find('"').map(|e| s2 + e) else { break };
                let name = a[s2..ne].to_string();
                if let Some(k) = a[ne..end].find("fmla=\"val ") {
                    let vs = ne + k + 10;
                    if let Some(ve) = a[vs..end].find('"').map(|e| vs + e) {
                        if let Ok(v) = a[vs..ve].trim().parse::<f32>() {
                            sp.adj.push((name, v));
                        }
                    }
                }
                at = ne;
            }
        }
    }
    // 塗りと線の色。**この図形の中の最初の物だけ**を見ます。
    //
    // 色は `srgbClr` で直に書いてあるとは限りません。テーマの名前
    // (`schemeClr`)と濃さの修飾(`lumMod` など)で書いてある方が普通です。
    // 解くのは [`crate::theme::dml_iro`] で、文書のテーマの配色を渡します
    let iro = |from: &str| -> Option<String> {
        let i = a.find(from)? + from.len();
        // その欄の終わりまで(影の色を線の色と読まないため)
        let owari = a[i..]
            .find(if from.starts_with("<a:ln") { "</a:ln>" } else { "</a:solidFill>" })
            .map(|e| i + e + 7)
            .unwrap_or(a.len());
        crate::theme::dml_iro(&a[i..owari], palette)
    };
    // **塗らないと言っている図形は塗りません**(`<a:noFill/>`)。
    //
    // 空要素は `<a:noFill />` と間を空けて書かれることもあります。片方しか
    // 見ていなかったので、内閣府の様式(document_4.docx)の枠が塗り潰され、
    // 紙が1枚まるごと濃い青になっていました(2026-09-03)。
    //
    // `<a:ln>` の中の色は**線の色**です。塗りとして読むと、線だけの図形が
    // その色で塗り潰されます
    let nofill = a.find("<a:noFill/>").or_else(|| a.find("<a:noFill />"));
    let ln_at = a.find("<a:ln ").or_else(|| a.find("<a:ln>"));
    let nuru = match (nofill, ln_at) {
        // 線より前の `noFill` は塗りの指定です
        (Some(i), Some(j)) => i > j,
        (Some(_), None) => false,
        (None, _) => true,
    };
    sp.fill = match (nuru, a.find("<a:solidFill>"), ln_at) {
        (false, _, _) => None,
        (true, Some(i), Some(j)) if i > j => None,
        (true, Some(_), _) => iro("<a:solidFill>"),
        (true, None, _) => None,
    };
    sp.line = iro("<a:ln ");
    // **線の種類**(`<a:prstDash val="dash"/>`)。無ければ実線
    if let Some(i) = a.find("<a:prstDash val=\"") {
        let s2 = i + 17;
        if let Some(e) = a[s2..].find('"') {
            let v = &a[s2..s2 + e];
            if v != "solid" && !v.is_empty() {
                sp.dash = Some(v.to_string());
            }
        }
    }
    if let Some(i) = a.find("<a:ln w=\"") {
        let s2 = i + 9;
        if let Some(e) = a[s2..].find('"') {
            if let Ok(w) = a[s2..s2 + e].parse::<f32>() {
                sp.line_w = w / 12_700.0;
            }
        }
    }
    // 不透明度・回転・反転・影
    if let Some(i) = a.find("<a:alpha val=\"") {
        let s2 = i + 14;
        if let Some(e) = a[s2..].find('"') {
            if let Ok(v) = a[s2..s2 + e].parse::<f32>() {
                sp.alpha = (v / 100_000.0).clamp(0.0, 1.0);
            }
        }
    }
    if let Some(i) = a.find("<a:xfrm rot=\"") {
        let s2 = i + 13;
        if let Some(e) = a[s2..].find('"') {
            if let Ok(v) = a[s2..s2 + e].parse::<f32>() {
                sp.rot = v / 60_000.0;
            }
        }
    }
    sp.flip_h = a.contains("flipH=\"1\"");
    sp.flip_v = a.contains("flipV=\"1\"");
    sp.shadow = a.contains("<a:outerShdw");
    // 図形の中の文字
    // **`<w:txbxContent>` の**後ろから**探します。** そのタグ自身が
    // `<w:t` で始まるので、頭から探すと自分に当たり、タグごと字として
    // 拾います(2026-08-29 に往復させて気づきました — 「往復」ではなく
    // `<w:p><w:r><w:t …>往復` が入りました)
    if let Some(i) = a.find("<w:txbxContent>").map(|i| i + "<w:txbxContent>".len()) {
        let owari = a[i..].find("</w:txbxContent>").map(|e| i + e).unwrap_or(a.len());
        let naka = &a[i..owari];
        let t = txbx_text(naka);
        if !t.is_empty() {
            sp.text = Some(t);
        }
        // **箱の中の書き方も読みます**(2026-09-01 発注者)。前は字だけを
        // 拾って `w:rPr` と `w:pPr` を捨てていたので、内閣府の調査票の
        // 担当欄が 9pt の決め打ちで組まれ、行も詰まっていました。
        //
        // 字の大きさは最初の `w:sz`(1/2 pt)。言っていなければ `None` の
        // ままで、描く側が文書の既定を当てます
        if let Some(pt) = hiroi(naka, "<w:sz w:val=\"") {
            sp.text_fmt.size_pt = Some(pt / 2.0);
        }
        // 書体の名前。行送りとベースラインの位置がこれで決まります
        if let Some(i) = naka.find("<w:rFonts ") {
            let e = naka[i..].find('>').map(|e| i + e).unwrap_or(naka.len());
            for k in ["w:eastAsia", "w:ascii"] {
                let v = attr_str(&naka[i..e], k);
                if !v.is_empty() {
                    sp.text_fmt.font = Some(v);
                    break;
                }
            }
        }
        // 行の高さは最初の `w:spacing`。`exact` と `atLeast` は twip の
        // 高さそのものです(`auto` は倍率なので、ここでは見ません)
        if let Some(j) = naka.find("<w:spacing ") {
            let e = naka[j..].find('>').map(|e| j + e).unwrap_or(naka.len());
            let tag = &naka[j..e];
            let rule = attr_str(tag, "w:lineRule");
            if matches!(rule.as_str(), "exact" | "atLeast") {
                if let Some(v) = attr_str(tag, "w:line").parse::<f32>().ok().filter(|v| *v > 0.0) {
                    sp.text_fmt.line_pt = Some(v / 20.0);
                }
            }
        }
    }
    Some(sp)
}

/// タグの属性を字で。無ければ空。
fn attr_str(tag: &str, key: &str) -> String {
    let pat = format!("{key}=\"");
    match tag.find(&pat) {
        Some(i) => {
            let s = i + pat.len();
            tag[s..].find('"').map(|e| tag[s..s + e].to_string()).unwrap_or_default()
        }
        None => String::new(),
    }
}

/// `<w:sz w:val="22"/>` のような、数を1つ拾う。無ければ `None`
fn hiroi(naka: &str, pat: &str) -> Option<f32> {
    let i = naka.find(pat)? + pat.len();
    let e = naka[i..].find('"')? + i;
    naka[i..e].parse().ok()
}

/// テキストボックスの中の字を**全部**拾う。段落の切れ目は改行にします。
///
/// **前は最初の `<w:t>` しか読んでいませんでした**(2026-08-30)。うちが
/// 書く図形は字が1つなので気づきませんでしたが、Word の作る欄は何行も
/// 持ちます。内閣府の告知書の窓口の欄は、5行のうち1行だけが残っていました。
fn txbx_text(naka: &str) -> String {
    let mut gyou: Vec<String> = Vec::new();
    for p in naka.split("</w:p>") {
        let mut s = String::new();
        let mut at = 0usize;
        while let Some(i) = p[at..].find("<w:t").map(|i| i + at) {
            let Some(k) = p[i..].find('>').map(|k| k + i + 1) else { break };
            // `<w:tab/>` などを `<w:t>` と読み違えない
            if !matches!(p[i + 4..].chars().next(), Some('>') | Some(' ')) {
                at = k;
                continue;
            }
            let Some(e) = p[k..].find("</w:t>").map(|e| e + k) else { break };
            s.push_str(&unesc(&p[k..e]));
            at = e + 6;
        }
        if !s.is_empty() {
            gyou.push(s);
        }
    }
    gyou.join("\n")
}


/// **`w:spacing` の行の高さを読む。** 返すのは (倍率, 高さ) の組です。
///
/// `w:lineRule="auto"`(既定)なら `w:line` は 240 = 1行 の倍率です。
/// `atLeast` と `exact` は twips の**高さそのもの**なので、pt に直して
/// そのまま持ちます。`exact` は組の2つめが `true` です。
///
/// 前はこの2つも 1行の高さ(`kumihan::LINE_MM`)で割って倍率にしていました。
/// 行の高さが 6.4mm の決め打ちだったので割れましたが、書体から出すように
/// 変えたので割れません(2026-09-01)。
fn gyou_bairitsu(line: Option<f32>, rule: Option<String>) -> (f32, Option<(f32, bool)>) {
    // **無指定は 0.0 です**(2026-09-01)。1.0 を入れると「1倍と言った」と
    // 見分けが付かず、python-docx が None を返す所でうちが 1.0 を返します
    let Some(v) = line else { return (0.0, None) };
    match rule.as_deref() {
        Some("atLeast") => (0.0, Some((v / 20.0, false))),
        Some("exact") => (0.0, Some((v / 20.0, true))),
        _ => ((v / 240.0).clamp(0.5, 5.0), None),
    }
}
