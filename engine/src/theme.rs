//! **テンプレート(テーマ)** — スタイル名 → 書式の表+ページ設定。
//!
//! 発注者 2026-08-16(SEKKEI「本文とテンプレートを分ける」): 本文は意味だけ、
//! 見た目はテンプレートが持つ。画面は常に「本文×テンプレート」の合成。
//! Word の失敗(直接書式が同じくらい簡単なら誰もスタイルを使わない)を
//! 設計で防ぎ、HTML 変換(本文×テンプレート = HTML×CSS)を成り立たせる。
//!
//! - 形は TOML の部分集合。**読み手は自前**(settings.toml・rpc の JSON と
//!   同じ流儀 — 依存を増やさない)。節 `[スタイル.見出し1]` と
//!   `鍵 = 値`(文字列・数・真偽)だけで、入れ子の表や配列は受けない
//! - 鍵は**日本語と英語の両方**を受ける(リボンの名乗り `札`/`label` と
//!   同じ前例)。AI が生成する成果物なので、どちらで書かれても読める
//! - [`compose`] が意味だけの [`Document`] の**写し**に書式を流し込む。
//!   一方通行 — 写しから意味を推測して戻すことはしない

use crate::doc::{Align, Block, Document, PageSetup, ParaStyle};

/// スタイル1つの書式。`None`/`false` は「テンプレートは指定しない」
/// (文書の既定に従う)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleDef {
    /// スタイル名。役割の分は固定名(本文・見出し1〜3・引用)。
    /// 利用者が新設した分は自由な名前で、段落の `style_id` が名指す
    pub name: String,
    pub size_pt: Option<f32>,
    pub font: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// 文字色 `RRGGBB`
    pub color: Option<String>,
    /// 段落の帯(背景色)`RRGGBB`
    pub shade: Option<String>,
    pub align: Option<Align>,
    pub space_before_pt: f32,
    pub space_after_pt: f32,
    /// 行間の倍率。`None` は指定なし
    pub line_spacing: Option<f32>,
}

/// **組み方** — 媒体の違いはここに集まる(発注者 2026-08-16
/// 「テンプレートだけ変更すれば Web やアプリビルダーも作れる。横幅可変・
/// ページ区切りなし。PowerPoint は文字がページを跨がらない。それだけのこと」)。
///
/// | 媒体 | 横幅 | 区切り |
/// |---|---|---|
/// | 紙(docx / PDF) | 固定(mm) | ページ |
/// | Web / アプリ | **可変**(窓の幅) | **なし**(1本の流れ) |
///
/// **「跨ぎ」はまだ持たない。** 発表(1節=1枚・字が枚を跨がない)の組み手が
/// 無いので、効かない切替を先に置かない(SEKKEI の決め)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Setting {
    /// 横幅が窓に従う(紙の幅を使わない)
    pub fluid: bool,
    /// ページに折らない(1本の長い流れ)
    pub endless: bool,
}

/// テンプレート。文書の頭の `:template: 名前` が名指す実体。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Theme {
    /// 組み方(紙 / Web)。既定は紙
    pub setting: Setting,
    /// 文書の既定の書体(`[文書]` の `書体`)
    pub font: Option<String>,
    /// 文書の既定の字の大きさ(`[文書]` の `大きさ`)
    pub size_pt: Option<f32>,
    /// ページ設定(`[ページ]`)。`None` はテンプレートが指定しない
    pub page: Option<PageSetup>,
    pub styles: Vec<StyleDef>,
}

impl Theme {
    /// 名前でスタイルを引く
    pub fn style(&self, name: &str) -> Option<&StyleDef> {
        self.styles.iter().find(|s| s.name == name)
    }

    /// 役割に対応する固定のスタイル名
    pub fn role_name(style: ParaStyle) -> Option<&'static str> {
        match style {
            ParaStyle::Body => Some("本文"),
            ParaStyle::Quote => Some("引用"),
            ParaStyle::Heading(1) => Some("見出し1"),
            ParaStyle::Heading(2) => Some("見出し2"),
            ParaStyle::Heading(_) => Some("見出し3"),
            // 目次・図表目次の行は「目次の更新」が作る物 — テンプレートの
            // スタイルでは(まだ)着せ替えない
            ParaStyle::Toc(_) | ParaStyle::Tof => None,
        }
    }
}

/// **同梱の既定のテンプレート。** 今までの新規文書の見た目
/// (writer の `set_para_style` が焼き付けていた 16/13/11.5pt 太字)を
/// そのまま写した物 — 既定テーマでの合成は、直接書式の時代と同じ紙面に
/// なる(段階Aの門番)。TOML の字で持つのは、**形式そのものを毎回
/// 通す**ため — 同梱だけ Rust の直書きだと、読み手の穴に気づけない
pub const DEFAULT_TOML: &str = r#"# officework の既定のテンプレート
[スタイル.見出し1]
大きさ = 16
太字 = true

[スタイル.見出し2]
大きさ = 13
太字 = true

[スタイル.見出し3]
大きさ = 11.5
太字 = true
"#;

/// 既定のテンプレートを読む(壊れていたらそれは不具合 — panic でよい)
pub fn default_theme() -> Theme {
    parse(DEFAULT_TOML).expect("同梱の既定テンプレートが読めない")
}

/// TOML(部分集合)からテンプレートを読む。
/// 知らない節・知らない鍵は**黙って捨てずに** Err で言う — テンプレートは
/// 人と AI が書く物で、綴りの間違いに黙ると「効かない」だけが残る
pub fn parse(src: &str) -> Result<Theme, String> {
    let mut th = Theme::default();
    // いま居る節。None = 頭(節の外)
    enum Sec {
        Setting,
        Doc,
        Page,
        Style(usize),
    }
    let mut cur: Option<Sec> = None;
    for (ln, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let name = name.trim();
            cur = Some(if name == "組み方" || name.eq_ignore_ascii_case("layout") {
                Sec::Setting
            } else if name == "文書" || name.eq_ignore_ascii_case("document") {
                Sec::Doc
            } else if name == "ページ" || name.eq_ignore_ascii_case("page") {
                Sec::Page
            } else if let Some(sn) = name
                .strip_prefix("スタイル.")
                .or_else(|| name.strip_prefix("style."))
            {
                th.styles.push(StyleDef { name: sn.trim().to_string(), ..Default::default() });
                Sec::Style(th.styles.len() - 1)
            } else {
                return Err(format!("{} 行目: 知らない節 [{name}](文書 / ページ / スタイル.名前)", ln + 1));
            });
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("{} 行目: 「鍵 = 値」の形ではありません: {line}", ln + 1));
        };
        let (k, v) = (k.trim(), v.trim());
        let s = |v: &str| -> Result<String, String> {
            v.strip_prefix('"')
                .and_then(|x| x.strip_suffix('"'))
                .map(|x| x.to_string())
                .ok_or_else(|| format!("{} 行目: 文字列は \"…\" で囲む: {v}", ln + 1))
        };
        let n = |v: &str| -> Result<f32, String> {
            v.parse::<f32>().map_err(|_| format!("{} 行目: 数が読めません: {v}", ln + 1))
        };
        let b = |v: &str| -> Result<bool, String> {
            match v {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("{} 行目: true か false: {v}", ln + 1)),
            }
        };
        match &cur {
            None => return Err(format!("{} 行目: 節の外に鍵があります: {k}", ln + 1)),
            Some(Sec::Setting) => match k {
                "横幅" | "width" => {
                    th.setting.fluid = match s(v)?.as_str() {
                        "可変" | "fluid" => true,
                        "固定" | "fixed" => false,
                        other => return Err(format!("{} 行目: 横幅は 可変 か 固定: {other}", ln + 1)),
                    }
                }
                "区切り" | "break" => {
                    th.setting.endless = match s(v)?.as_str() {
                        "なし" | "none" => true,
                        "ページ" | "page" => false,
                        other => {
                            return Err(format!("{} 行目: 区切りは ページ か なし: {other}", ln + 1))
                        }
                    }
                }
                _ => return Err(format!("{} 行目: [組み方] の知らない鍵: {k}", ln + 1)),
            },
            Some(Sec::Doc) => match k {
                "書体" | "font" => th.font = Some(s(v)?),
                "大きさ" | "size" => th.size_pt = Some(n(v)?),
                _ => return Err(format!("{} 行目: [文書] の知らない鍵: {k}", ln + 1)),
            },
            Some(Sec::Page) => {
                let p = th.page.get_or_insert_with(PageSetup::default);
                match k {
                    "用紙" | "paper" => match s(v)?.as_str() {
                        "A4" => {}
                        "A4横" | "A4 landscape" => {
                            (p.w_mm, p.h_mm) = (297.0, 210.0);
                        }
                        "A3" => (p.w_mm, p.h_mm) = (297.0, 420.0),
                        "B5" => (p.w_mm, p.h_mm) = (182.0, 257.0),
                        other => return Err(format!("{} 行目: 知らない用紙: {other}(A4 / A4横 / A3 / B5)", ln + 1)),
                    },
                    "余白" | "margin" => {
                        let m = n(v)?;
                        (p.left_mm, p.right_mm, p.top_mm, p.bottom_mm) = (m, m, m, m);
                    }
                    "段組み" | "columns" => p.columns = n(v)? as u8,
                    _ => return Err(format!("{} 行目: [ページ] の知らない鍵: {k}", ln + 1)),
                }
            }
            Some(Sec::Style(i)) => {
                let d = &mut th.styles[*i];
                match k {
                    "大きさ" | "size" => d.size_pt = Some(n(v)?),
                    "書体" | "font" => d.font = Some(s(v)?),
                    "太字" | "bold" => d.bold = b(v)?,
                    "斜体" | "italic" => d.italic = b(v)?,
                    "下線" | "underline" => d.underline = b(v)?,
                    "色" | "color" => d.color = Some(s(v)?),
                    "帯" | "shade" => d.shade = Some(s(v)?),
                    "揃え" | "align" => {
                        d.align = Some(match s(v)?.as_str() {
                            "左" | "left" => Align::Left,
                            "中央" | "center" => Align::Center,
                            "右" | "right" => Align::Right,
                            "両端" | "justify" => Align::Justify,
                            "均等" | "distribute" => Align::Distribute,
                            other => return Err(format!("{} 行目: 知らない揃え: {other}", ln + 1)),
                        });
                    }
                    "前の空き" | "space_before" => d.space_before_pt = n(v)?,
                    "後の空き" | "space_after" => d.space_after_pt = n(v)?,
                    "行間" | "line_spacing" => d.line_spacing = Some(n(v)?),
                    _ => return Err(format!("{} 行目: [スタイル] の知らない鍵: {k}", ln + 1)),
                }
            }
        }
    }
    Ok(th)
}

/// テンプレート → TOML(正規形)。**AI と人が読み書きする物**なので、
/// 鍵は日本語で書き、並びは [`parse`] が読む順に揃える。
/// 門番は `parse(write(x)) == x`(往復)
pub fn write(th: &Theme) -> String {
    let mut s = String::new();
    if th.setting != Setting::default() {
        s.push_str("[組み方]\n");
        if th.setting.fluid {
            s.push_str("横幅 = \"可変\"\n");
        }
        if th.setting.endless {
            s.push_str("区切り = \"なし\"\n");
        }
        s.push('\n');
    }
    if th.font.is_some() || th.size_pt.is_some() {
        s.push_str("[文書]\n");
        if let Some(f) = &th.font {
            s.push_str(&format!("書体 = {f:?}\n"));
        }
        if let Some(n) = th.size_pt {
            s.push_str(&format!("大きさ = {}\n", num(n)));
        }
        s.push('\n');
    }
    if let Some(p) = &th.page {
        s.push_str("[ページ]\n");
        let paper = match (p.w_mm, p.h_mm) {
            (297.0, 210.0) => Some("A4横"),
            (297.0, 420.0) => Some("A3"),
            (182.0, 257.0) => Some("B5"),
            (210.0, 297.0) => Some("A4"),
            _ => None,
        };
        if let Some(k) = paper {
            s.push_str(&format!("用紙 = {k:?}\n"));
        }
        // 4辺が同じときだけ「余白」で書ける(違う値は今の器が持てない)
        if p.left_mm == p.right_mm && p.left_mm == p.top_mm && p.left_mm == p.bottom_mm {
            s.push_str(&format!("余白 = {}\n", num(p.left_mm)));
        }
        if p.columns > 1 {
            s.push_str(&format!("段組み = {}\n", p.columns));
        }
        s.push('\n');
    }
    for d in &th.styles {
        s.push_str(&format!("[スタイル.{}]\n", d.name));
        if let Some(n) = d.size_pt {
            s.push_str(&format!("大きさ = {}\n", num(n)));
        }
        if let Some(f) = &d.font {
            s.push_str(&format!("書体 = {f:?}\n"));
        }
        if d.bold {
            s.push_str("太字 = true\n");
        }
        if d.italic {
            s.push_str("斜体 = true\n");
        }
        if d.underline {
            s.push_str("下線 = true\n");
        }
        if let Some(c) = &d.color {
            s.push_str(&format!("色 = {c:?}\n"));
        }
        if let Some(c) = &d.shade {
            s.push_str(&format!("帯 = {c:?}\n"));
        }
        if let Some(a) = d.align {
            let k = match a {
                Align::Left => "左",
                Align::Center => "中央",
                Align::Right => "右",
                Align::Justify => "両端",
                Align::Distribute => "均等",
            };
            s.push_str(&format!("揃え = {k:?}\n"));
        }
        if d.space_before_pt != 0.0 {
            s.push_str(&format!("前の空き = {}\n", num(d.space_before_pt)));
        }
        if d.space_after_pt != 0.0 {
            s.push_str(&format!("後の空き = {}\n", num(d.space_after_pt)));
        }
        if let Some(l) = d.line_spacing {
            s.push_str(&format!("行間 = {}\n", num(l)));
        }
        s.push('\n');
    }
    while s.ends_with("\n\n") {
        s.pop();
    }
    s
}

/// 数を素直に書く(整数は小数点を出さない — 人が読む物なので)
fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// **合成** — 意味だけの文書の写しに、テンプレートの書式を流し込む。
///
/// 返った写しは `layout` にそのまま渡せる(組版エンジンは無傷)。
/// 一方通行: 写しを保存には使わない(保存は意味の側)。
///
/// 流し込みの規則 — **本文が指定していない所だけ埋める**(run の
/// `size_pt: None` の意味論と同じ)。ネイティブ文書は見た目の欄が常に
/// 空なので全部テンプレートから来るが、互換の文書に掛けても直接書式を
/// 潰さない。
pub fn compose(doc: &Document, theme: &Theme) -> Document {
    let mut out = doc.clone();
    if out.font.is_none() {
        out.font = theme.font.clone();
    }
    if out.size_pt.is_none() {
        out.size_pt = theme.size_pt;
    }
    if out.page.is_none() {
        out.page = theme.page.clone();
    }
    for block in &mut out.blocks {
        let crate::doc::Block::Para(para) = block else { continue };
        // 名指しのスタイル(style_id)が役割の固定名より勝つ —
        // 利用者が新設した物は名前で着る
        let def = para
            .style_id
            .as_deref()
            .and_then(|id| theme.style(id))
            .or_else(|| Theme::role_name(para.style).and_then(|n| theme.style(n)));
        let Some(def) = def else { continue };
        if let Some(a) = def.align {
            para.align = a;
        }
        if para.shade.is_none() {
            para.shade = def.shade.clone();
        }
        if para.space_before_pt == 0.0 {
            para.space_before_pt = def.space_before_pt;
        }
        if para.space_after_pt == 0.0 {
            para.space_after_pt = def.space_after_pt;
        }
        if let Some(ls) = def.line_spacing {
            if para.line_spacing == 1.0 {
                para.line_spacing = ls;
            }
        }
        for r in &mut para.runs {
            if r.size_pt.is_none() {
                r.size_pt = def.size_pt;
            }
            if r.font.is_none() {
                r.font = def.font.clone();
            }
            r.fmt.bold |= def.bold;
            r.fmt.italic |= def.italic;
            r.fmt.underline |= def.underline;
            if r.fmt.color.is_none() {
                r.fmt.color = def.color.clone();
            }
        }
    }
    // **文字単位のスタイル**(2026-08-16)。段落のを流し込んだ後に掛ける —
    // 字に付いた名前の方が、段落の名前より内側にあるので勝つ。
    // 段落だけの項目(帯・揃え・空き・行間)は字には効かない
    for block in &mut out.blocks {
        let Block::Para(para) = block else { continue };
        for r in &mut para.runs {
            let Some(name) = r.fmt.style_id.clone() else { continue };
            let Some(def) = theme.style(&name) else { continue };
            if let Some(s) = def.size_pt {
                r.size_pt = Some(s);
            }
            if def.font.is_some() {
                r.font = def.font.clone();
            }
            r.fmt.bold |= def.bold;
            r.fmt.italic |= def.italic;
            r.fmt.underline |= def.underline;
            if def.color.is_some() {
                r.fmt.color = def.color.clone();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Paragraph, Run};

    fn 意味だけの文書() -> Document {
        let mut d = Document::default();
        let mut h = Paragraph::default();
        h.style = ParaStyle::Heading(1);
        h.runs.push(Run { text: "題".into(), size_pt: None, font: None, fmt: Default::default() });
        let mut b = Paragraph::default();
        b.runs.push(Run { text: "本文の字。".into(), size_pt: None, font: None, fmt: Default::default() });
        d.blocks = vec![crate::doc::Block::Para(h), crate::doc::Block::Para(b)];
        d
    }

    #[test]
    fn 既定テーマは直接書式の時代と同じ見た目を作る() {
        // **段階Aの門番。** writer の set_para_style が焼き付けていた
        // 16/13/11.5pt 太字と同じ値が、合成から出ること
        let d = compose(&意味だけの文書(), &default_theme());
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(16.0), "見出し1は16pt");
        assert!(ps[0].runs[0].fmt.bold, "見出し1は太字");
        assert_eq!(ps[1].runs[0].size_pt, None, "本文は既定のまま(焼き付けない)");
        assert!(!ps[1].runs[0].fmt.bold);
    }

    #[test]
    fn 合成は元の文書を触らない() {
        let d = 意味だけの文書();
        let _ = compose(&d, &default_theme());
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, None, "意味の側は意味のまま");
    }

    #[test]
    fn 直接書式は潰さない() {
        // 互換の文書に掛けても、本文が指定した見た目が勝つ
        let mut d = 意味だけの文書();
        if let crate::doc::Block::Para(p) = &mut d.blocks[0] {
            p.runs[0].size_pt = Some(22.0);
        }
        let out = compose(&d, &default_theme());
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(22.0), "直接の 22pt が残る");
    }

    #[test]
    fn 名指しのスタイルが役割より勝つ() {
        let mut th = default_theme();
        th.styles.push(StyleDef {
            name: "注意書き".into(),
            color: Some("C7433F".into()),
            ..Default::default()
        });
        let mut d = 意味だけの文書();
        if let crate::doc::Block::Para(p) = &mut d.blocks[1] {
            p.style_id = Some("注意書き".into());
        }
        let out = compose(&d, &th);
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[1].runs[0].fmt.color.as_deref(), Some("C7433F"));
    }

    #[test]
    fn 文字のスタイルは段落のより内側で勝つ() {
        let mut th = default_theme();
        th.styles.push(StyleDef {
            name: "注意".into(),
            color: Some("C7433F".into()),
            size_pt: Some(14.0),
            ..Default::default()
        });
        let mut d = 意味だけの文書();
        if let crate::doc::Block::Para(p) = &mut d.blocks[0] {
            // 見出し1(16pt)の中の1語だけ「注意」
            p.runs[0].fmt.style_id = Some("注意".into());
        }
        let out = compose(&d, &th);
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(14.0), "字の名前が段落の名前に負けた");
        assert_eq!(ps[0].runs[0].fmt.color.as_deref(), Some("C7433F"));
        assert!(ps[0].runs[0].fmt.bold, "段落の太字は残る(字の側が外していない)");
    }

    #[test]
    fn 知らない鍵は黙らない() {
        assert!(parse("[スタイル.x]\n大きき = 16\n").is_err(), "綴りの間違いに黙ると「効かない」だけが残る");
        assert!(parse("[謎の節]\n").is_err());
        assert!(parse("大きさ = 16\n").is_err(), "節の外の鍵");
    }

    #[test]
    fn 日本語と英語の鍵を同じに読む() {
        let ja = parse("[スタイル.見出し1]\n大きさ = 16\n太字 = true\n").unwrap();
        let en = parse("[style.見出し1]\nsize = 16\nbold = true\n").unwrap();
        assert_eq!(ja, en);
    }

    #[test]
    fn テンプレートが往復する() {
        // **門番**: 書いて読むと同じ物になる(AI が書いた物も、画面が
        // 足したスタイルも、同じ表を通る)
        let src = "[文書]\n大きさ = 11\n\n[ページ]\n用紙 = \"B5\"\n余白 = 15\n\n\
                   [スタイル.見出し1]\n大きさ = 20\n太字 = true\n色 = \"1B6E3C\"\n後の空き = 8\n";
        let th = parse(src).unwrap();
        let back = write(&th);
        assert_eq!(parse(&back).unwrap(), th, "往復で崩れた:\n{back}");
    }

    #[test]
    fn 組み方の値が往復する() {
        let th = parse("[組み方]\n横幅 = \"可変\"\n区切り = \"なし\"\n").unwrap();
        assert!(th.setting.fluid && th.setting.endless);
        assert_eq!(parse(&write(&th)).unwrap(), th);
        // 既定(紙)は書かない — 空の節を増やさない
        assert_eq!(write(&Theme::default()), "");
        assert!(parse("[組み方]\n横幅 = \"なんとなく\"\n").is_err(), "知らない値に黙った");
    }

    #[test]
    fn ページの節が読める() {
        let th = parse("[ページ]\n用紙 = \"B5\"\n余白 = 15\n").unwrap();
        let p = th.page.unwrap();
        assert_eq!((p.w_mm, p.h_mm), (182.0, 257.0));
        assert_eq!(p.left_mm, 15.0);
    }
}
