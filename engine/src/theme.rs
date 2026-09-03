//! **テンプレート(テーマ)** — スタイル名 → 書式の表+ページ設定。
//!
//! 発注者 2026-08-16(SEKKEI「本文とテンプレートを分ける」): 本文は意味だけ、
//! 見た目はテンプレートが持つ。画面は常に「本文×テンプレート」の合成。
//! Word の失敗(直接書式が同じくらい簡単なら誰もスタイルを使わない)を
//! 設計で防ぎ、HTML 変換(本文×テンプレート = HTML×CSS)を成り立たせる。
//!
//! - 形は TOML の部分集合。**読み手は自前**(settings.toml・rpc の JSON と
//!   同じ流儀 — 依存を増やさない)。節 `[スタイル.見出し1]` と
//!   `キー = 値`(文字列・数・真偽)だけで、入れ子の表や配列は受けない
//! - キーは**日本語と英語の両方**を受ける(リボンの名乗り `札`/`label` と
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
    /// 段落の背景色 `RRGGBB`(docx の網かけ)
    pub shade: Option<String>,
    pub align: Option<Align>,
    pub space_before_pt: f32,
    pub space_after_pt: f32,
    /// 行間の倍率。`None` は指定なし
    pub line_spacing: Option<f32>,
    /// **1行目の字下げ**(全角の文字数)。日本語の本文は1字下げるのが普通。
    /// `None` は指定なし。負の値(ぶら下げ)は受けない
    pub first_line_chars: Option<f32>,
}

/// **組み方** — 媒体の違いはここに集まる(発注者 2026-08-16
/// 「テンプレートだけ変更すれば Web やアプリビルダーも作れる。横幅可変・
/// ページ区切りなし。PowerPoint は文字がページを跨がらない。それだけのこと」)。
///
/// | 媒体 | 横幅 | 区切り | 跨ぎ |
/// |---|---|---|---|
/// | 紙(docx / PDF) | 固定(mm) | ページ | 跨ぐ |
/// | Web / アプリ | **可変**(窓の幅) | **なし**(1本の流れ) | — |
/// | 発表(スライド) | 固定 | **節**(見出し1 ごと) | **跨がない** |
///
/// 3値とも 2026-08-17 に入った。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Break {
    /// 紙のように、入る分で折る
    #[default]
    Page,
    /// 折らない(1本の長い流れ = Web)
    None,
    /// **節ごとに1枚**(見出し1 で改める = 発表)
    Section,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Setting {
    /// 横幅が窓に従う(紙の幅を使わない)
    pub fluid: bool,
    /// 区切り方
    pub br: Break,
    /// **字が枚を跨がない**(入りきらない段落は丸ごと次へ送る)。
    /// 発表の資料の作法 — 1つの文が2枚に割れると読めない
    pub keep: bool,
}

impl Setting {
    /// 折らない(Web の流し組み)
    pub fn endless(&self) -> bool {
        self.br == Break::None
    }
    /// 節ごとに1枚(発表)
    pub fn per_section(&self) -> bool {
        self.br == Break::Section
    }
}

/// その言語だけの、文書の既定。`[文書.ko]` のような節で書きます。
///
/// `None` は「この言語では言わない」で、`[文書]` の分がそのまま効きます。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LangDoc {
    pub font: Option<String>,
    pub size_pt: Option<f32>,
    /// その言語の既定の用紙(幅 mm, 高さ mm)。**アメリカだけレターです**
    /// (2026-08-30 発注者)。他の言語は A4 なので、書かなければ `None` です
    pub paper_mm: Option<(f32, f32)>,
}

/// 用紙の名前を mm に直す。**読む側と書く側で同じ表を見ます**
pub fn youshi_mm(na: &str) -> Option<(f32, f32)> {
    match na {
        "A4" => Some((210.0, 297.0)),
        "A4横" | "A4 landscape" => Some((297.0, 210.0)),
        "A3" => Some((297.0, 420.0)),
        "B5" => Some((182.0, 257.0)),
        // アメリカの標準の用紙。8.5 × 11 インチ
        "レター" | "Letter" => Some((215.9, 279.4)),
        _ => None,
    }
}

/// mm を用紙の名前に戻す。**[`youshi_mm`] と往復できます**
pub fn youshi_na(wh: (f32, f32)) -> &'static str {
    for na in ["A4", "A4横", "A3", "B5", "レター"] {
        if youshi_mm(na) == Some(wh) {
            return na;
        }
    }
    "A4"
}

fn youshi_error(na: &str) -> String {
    format!("知らない用紙: {na}(A4 / A4横 / A3 / B5 / レター)")
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
    /// **言語ごとの `[文書]`**(`[文書.ko]`)。
    ///
    /// 発注者 2026-08-26「PCやディレクトリーの標準テンプレートには、
    /// フォントやサイズが言語によって違うことを考慮しないといけない」。
    ///
    /// 書体が言語で違うのは字が無いからです。大きさも違います —
    /// 日本語の本文は 10.5pt、英語は 11pt、ベトナム語の公文書は 13pt が
    /// 普通で、同じ数字にすると読みにくくなります。
    ///
    /// 中身は (言語の札, その言語の既定)。[`for_language`](Theme::for_language)
    /// が畳みます。
    pub lang_docs: Vec<(String, LangDoc)>,
    /// ページ設定(`[ページ]`)。`None` はテンプレートが指定しない
    pub page: Option<PageSetup>,
    /// 記入欄の送り先(`[送り先]`)。アプリの形で書き出すときに使います。
    /// **どこへ送るかも見た目と同じくテンプレートの持ち物**です — 同じ
    /// 記入用紙を、試験の宛先と本番の宛先で使い分けられます
    pub submit: Option<Submit>,
    /// ページの飾り(`[ページ]` の中)。**ページの飾りは見た目なので
    /// テンプレートの持ち物です**(2026-08-18)。前は文書の側にしか無く、
    /// adoc で保存すると消えていました。
    ///
    /// ヘッダーとフッターの中では `{ページ}` と `{ページ数}` がその頁の
    /// 数字になります。
    pub header: Option<String>,
    pub footer: Option<String>,
    pub watermark: Option<String>,
    /// ページの色 `RRGGBB`
    pub page_color: Option<String>,
    /// 縦書き(右の列から左へ)
    pub vertical: bool,
    pub styles: Vec<StyleDef>,
    /// **様式(セル)**。申請書のような枠の書類の形です(2026-08-18)。
    /// 中身は本文のラベル付きリスト(`項目:: 値`)が持ち、ここは枠だけを
    /// 持ちます。結び付けは名前で取ります
    pub forms: Vec<Form>,
}

/// 様式(セル)1つ。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Form {
    pub name: String,
    pub rows: Vec<FormRow>,
}

/// 様式の1行。`セル` に項目の名前を並べ、`幅` があれば桁の比になります。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormRow {
    /// この行に並べる項目の名前
    pub cells: Vec<String>,
    /// 桁の幅の比。空なら等分
    pub widths: Vec<f32>,
}

/// 記入した内容の送り先。
#[derive(Debug, Clone, PartialEq)]
pub struct Submit {
    /// 送り先(`action`)
    pub action: String,
    /// 送り方。`"post"` か `"get"`
    pub method: String,
    /// 送るボタンの文字
    pub label: String,
}

impl Default for Submit {
    fn default() -> Self {
        Self { action: String::new(), method: "post".into(), label: "送信".into() }
    }
}

impl Theme {
    /// 名前でスタイルを引く
    pub fn style(&self, name: &str) -> Option<&StyleDef> {
        self.styles.iter().find(|s| s.name == name)
    }

    /// その言語に当たる `[文書.xx]`。
    ///
    /// 探し方は札そのもの(`pt-br`)が先で、無ければ言語だけ(`pt`)です。
    /// ブラジルの分を書いてあればそちらを、書いていなければポルトガル語の
    /// 分を使う、ということです。
    pub fn lang_doc(&self, lang: &str) -> Option<&LangDoc> {
        let base = lang.split('-').next().unwrap_or(lang);
        self.lang_docs
            .iter()
            .find(|(t, _)| t.eq_ignore_ascii_case(lang))
            .or_else(|| self.lang_docs.iter().find(|(t, _)| t.eq_ignore_ascii_case(base)))
            .map(|(_, d)| d)
    }

    /// いま画面が使っている言語で畳んだ写し。
    ///
    /// **重ねる前に、段ごとに畳んでください。** 先に重ねると、綴りの
    /// テンプレートが `[文書.en]` でしか大きさを言っていないとき、その
    /// 大きさが下の段の大きさに埋められて効かなくなります。
    pub fn for_current_language(&self) -> Theme {
        self.for_language(&crate::font::default_language())
    }

    /// **その言語の分を畳んだ写し。**
    ///
    /// `[文書.ko]` に書いてあることで `[文書]` を上書きします。使う側は
    /// これを通してから `font` と `size_pt` を見てください。
    pub fn for_language(&self, lang: &str) -> Theme {
        let mut th = self.clone();
        if let Some(d) = self.lang_doc(lang) {
            if d.font.is_some() {
                th.font = d.font.clone();
            }
            if d.size_pt.is_some() {
                th.size_pt = d.size_pt;
            }
            // **用紙も言語で変わります。** アメリカ英語だけレターです
            // (2026-08-30 発注者「en は、イギリスとアメリカの2つにして」)
            if let Some((w, h)) = d.paper_mm {
                let p = th.page.get_or_insert_with(crate::doc::PageSetup::default);
                (p.w_mm, p.h_mm) = (w, h);
            }
        }
        th
    }

    /// 役割に対応する固定のスタイル名
    pub fn role_name(style: ParaStyle) -> Option<&'static str> {
        match style {
            ParaStyle::Body => Some("本文"),
            ParaStyle::Title => Some("表題"),
            ParaStyle::Quote => Some("引用"),
            ParaStyle::Heading(1) => Some("見出し1"),
            ParaStyle::Heading(2) => Some("見出し2"),
            ParaStyle::Heading(3) => Some("見出し3"),
            ParaStyle::Heading(4) => Some("見出し4"),
            ParaStyle::Heading(_) => Some("見出し5"),
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

# **本文の大きさは言語で違います**(2026-08-26 発注者)。同じ 11pt でも、
# 日本語は大きく見え、ベトナム語は記号が潰れます。それぞれの国で
# 普通とされている大きさを既定にします。
#
# 書体はここでは決めません。機械にある物から選ぶので(kumihan::font の
# default_family)、テンプレートに名前を書くとかえって外れます。
[文書]
大きさ = 10.5

# 英語・ドイツ語などラテン文字の言語。Word の既定が 11pt
#
# **英語は国まで名乗ります**(2026-08-30 発注者)。アメリカだけ用紙が
# レター(8.5 × 11 インチ)で、イギリスは他と同じ A4 です
[文書.en-us]
大きさ = 11
用紙 = "レター"
[文書.en-gb]
大きさ = 11
[文書.de]
大きさ = 11
[文書.es]
大きさ = 11
[文書.fr]
大きさ = 11
[文書.it]
大きさ = 11
[文書.id]
大きさ = 11
[文書.pt]
大きさ = 11
[文書.pt-br]
大きさ = 11
[文書.tr]
大きさ = 11
[文書.ru]
大きさ = 11

# ベトナム語の公文書は 13pt(声調の記号が二重に付くので、小さいと潰れます)
[文書.vi]
大きさ = 13

# 韓国語は 10pt
[文書.ko]
大きさ = 10

# 中国語は簡体が五号(10.5pt)、繁体は 12pt
[文書.zh]
大きさ = 10.5
[文書.zh-tw]
大きさ = 12

[スタイル.表題]
大きさ = 20
太字 = true
後の空き = 10

[スタイル.見出し1]
大きさ = 16
太字 = true

[スタイル.見出し2]
大きさ = 13
太字 = true

[スタイル.見出し3]
大きさ = 11.5
太字 = true

[スタイル.引用]
斜体 = true
色 = "444444"

# **本家の AsciiDoc の書き方の見た目。** うちでは編集できませんが、
# 開いたときに「そういう塊だ」と分かるように既定を置きます
[スタイル.註記]
太字 = true
背景色 = "FFF6E0"

# 註記の仲間(TIP: / IMPORTANT: / WARNING: / CAUTION:)。
# 印ごとに別のスタイルなので、色を分けられます
[スタイル.ヒント]
太字 = true
背景色 = "E8F6EC"

[スタイル.重要]
太字 = true
背景色 = "FFF0F0"

[スタイル.警告]
太字 = true
色 = "9C2B2B"
背景色 = "FFF0F0"

[スタイル.注意]
太字 = true
背景色 = "FFF6E0"

[スタイル.塊の区切り]
色 = "8A8A8A"

[スタイル.塊の中]
書体 = "Noto Sans Mono CJK JP"

# 塊の種類ごとの見た目(2026-09-03)。名前は読み手が付けます
[スタイル.コードの塊]
書体 = "Noto Sans Mono CJK JP"
背景色 = "F4F6F8"

[スタイル.字のまま出す塊]
書体 = "Noto Sans Mono CJK JP"

# 字下げの段落(頭が空白の段落。字のまま組む物)
[スタイル.字下げ]
書体 = "Noto Sans Mono CJK JP"

[スタイル.そのまま通す塊]
書体 = "Noto Sans Mono CJK JP"
色 = "8A8A8A"

[スタイル.例の塊]
背景色 = "FBFBF3"

[スタイル.傍注の塊]
背景色 = "EEF3F6"

[スタイル.詩の塊]
斜体 = true

[スタイル.註記の塊]
背景色 = "FFF6E0"

[スタイル.ヒントの塊]
背景色 = "E8F6EC"

[スタイル.重要の塊]
背景色 = "FFF0F0"

[スタイル.警告の塊]
色 = "9C2B2B"
背景色 = "FFF0F0"

[スタイル.注意の塊]
背景色 = "FFF6E0"

# 行の中の等幅(`字`)。塊の中と同じ書体にします
[スタイル.等幅]
書体 = "Noto Sans Mono CJK JP"

[スタイル.塊の題]
太字 = true
色 = "444444"

[スタイル.説明のリスト]
太字 = true

[スタイル.取り込み]
色 = "8A8A8A"
斜体 = true

[スタイル.チェック]
色 = "1B6E3C"

[スタイル.見出し4]
大きさ = 11
太字 = true

[スタイル.見出し5]
大きさ = 10.5
太字 = true

# 塊に掛かる指定の行([source,python] など)。うちでは編集できませんが、
# 掛かっている塊が分かるように薄く出します
[スタイル.指定の行]
色 = "8A8A8A"

[スタイル.横の区切り線]
色 = "8A8A8A"

[スタイル.覚え書き]
色 = "9AA0A6"
斜体 = true
"#;

/// 既定のテンプレートを読む(壊れていたらそれは不具合 — panic でよい)
pub fn default_theme() -> Theme {
    // **いまの言語の分まで畳んで返します。** 呼ぶ側は「同梱の既定」と
    // 思って使うので、そこで言語を考えろというのは無理があります
    parse(DEFAULT_TOML)
        .expect("同梱の既定テンプレートが読めない")
        .for_current_language()
}

/// 利用者の標準テンプレートの置き場(`~/.config/officework/テンプレート.toml`)。
///
/// **書式の標準は3段です**(2026-08-26 発注者)。
///
/// . *文書* — その文章が自分で持つ(`Document::font` など)
/// . *綴り* — フォルダの `テンプレート.toml`
/// . *利用者* — ここ。**この機械で自分がいつも使う書式**
///
/// 下の段は、上の段が言っていないことだけを埋めます。どれも言っていな
/// ければ同梱の既定です。
pub fn user_template_name() -> &'static str {
    "テンプレート.toml"
}

/// テンプレートを1つ、重ねずにそのまま読む。無い・壊れているなら `None`。
///
/// **その段が自分で何を言っているかを見るため**に要ります。重ねた後の
/// テンプレートでは、下の段の言い分が上の段の言い分に見えてしまいます。
/// 「書式の標準」の画面は、どの段が効いているかを見せるのが役目なので、
/// そこでは重ねる前を使います。
pub fn read_theme(at: &std::path::Path) -> Option<Theme> {
    parse(&std::fs::read_to_string(at).ok()?).ok()
}

/// テンプレートの字に、節とキーを1つ書き入れた字を返す。
///
/// **人が手で書いた行を残します。** 読んで組み直すと、注釈も並び順も
/// 消えてしまいます。テンプレートは手で書く物なので、それは困ります。
///
/// - その節にそのキーがあれば、**その行だけ**書き替えます
/// - 節はあってキーが無ければ、節の終わりに足します
/// - 節が無ければ、末尾に節ごと足します
///
/// `値` は TOML の値としてそのまま書きます。文字列なら `"…"` で囲んだ字を
/// 渡してください。
pub fn put(src: &str, section: &str, key: &str, value: &str) -> String {
    let line = format!("{key} = {value}");
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut done = false;
    let mut saw_section = false;
    for l in src.lines() {
        let t = l.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            // 目当ての節から出るところ。まだ書けていなければここで足す
            if in_section && !done {
                out.push(line.clone());
                done = true;
            }
            in_section = name.trim() == section;
            saw_section |= in_section;
            out.push(l.to_string());
            continue;
        }
        if in_section && !done {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    out.push(line.clone());
                    done = true;
                    continue;
                }
            }
        }
        out.push(l.to_string());
    }
    if !done {
        if !saw_section {
            if !out.is_empty() {
                out.push(String::new());
            }
            out.push(format!("[{section}]"));
        }
        out.push(line);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// テンプレートの字から、節の中のキーを1行だけ消した字を返す。
///
/// [`put`] と対です。太字を外したときに `太字 = true` の行が残っていては
/// 困るので、無い物は行ごと消します。節やキーが無ければ何もしません。
pub fn drop_key(src: &str, section: &str, key: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut in_section = false;
    for l in src.lines() {
        let t = l.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = name.trim() == section;
            out.push(l);
            continue;
        }
        if in_section {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    continue;
                }
            }
        }
        out.push(l);
    }
    let mut s = out.join("\n");
    if src.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// 節の名前を書き替えた字を返す(`[スタイル.旧]` → `[スタイル.新]`)。
///
/// 中の行は触りません。節が無ければそのまま返します。
pub fn rename_section(src: &str, from: &str, to: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in src.lines() {
        let t = l.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if name.trim() == from {
                out.push(format!("[{to}]"));
                continue;
            }
        }
        out.push(l.to_string());
    }
    let mut s = out.join("\n");
    if src.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// スタイル1つの節の名前(`スタイル.名前`)。
pub fn style_section(name: &str) -> String {
    format!("スタイル.{name}")
}

/// スタイル1つを字に書き入れた字を返す。
///
/// 持っている項目は [`put`] で書き、持っていない項目は [`drop_key`] で
/// 消します。人が手で書いた注釈と並び順は残ります。
pub fn put_style(src: &str, def: &StyleDef) -> String {
    let section = style_section(&def.name);
    let lines = style_lines(def);
    let mut s = src.to_string();
    for key in STYLE_KEYS {
        match lines.iter().find(|(k, _)| k == key) {
            Some((_, v)) => s = put(&s, &section, key, v),
            None => s = drop_key(&s, &section, key),
        }
    }
    // 何も持たないスタイルでも、節だけは残します(名前を選べるように)
    if lines.is_empty() && !s.lines().any(|l| l.trim() == format!("[{section}]")) {
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&format!("[{section}]\n"));
    }
    s
}

/// `[スタイル.名前]` に書けるキーの並び。[`put_style`] と [`write`] が
/// 同じ表を見ます
const STYLE_KEYS: &[&str] = &[
    "大きさ", "書体", "太字", "斜体", "下線", "色", "背景色", "揃え",
    "前の空き", "後の空き", "行間", "字下げ",
];

/// スタイル1つを `キー = 値` の並びにする。**指定の無い項目は出しません**
fn style_lines(d: &StyleDef) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    if let Some(n) = d.size_pt {
        v.push(("大きさ", num(n)));
    }
    if let Some(f) = &d.font {
        v.push(("書体", format!("{f:?}")));
    }
    if d.bold {
        v.push(("太字", "true".into()));
    }
    if d.italic {
        v.push(("斜体", "true".into()));
    }
    if d.underline {
        v.push(("下線", "true".into()));
    }
    if let Some(c) = &d.color {
        v.push(("色", format!("{c:?}")));
    }
    if let Some(c) = &d.shade {
        v.push(("背景色", format!("{c:?}")));
    }
    if let Some(a) = d.align {
        let k = match a {
            Align::Left => "左",
            Align::Center => "中央",
            Align::Right => "右",
            Align::Justify => "両端",
            Align::Distribute => "均等",
        };
        v.push(("揃え", format!("{k:?}")));
    }
    if d.space_before_pt != 0.0 {
        v.push(("前の空き", num(d.space_before_pt)));
    }
    if d.space_after_pt != 0.0 {
        v.push(("後の空き", num(d.space_after_pt)));
    }
    if let Some(l) = d.line_spacing {
        v.push(("行間", num(l)));
    }
    if let Some(f) = d.first_line_chars {
        v.push(("字下げ", num(f)));
    }
    v
}

/// 書式を直すときの、**書き先の元**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// いま着ているファイルをその場で書き替える
    InPlace,
    /// 配られたファイルの写しを作る(中身はそのファイルから)
    CopyOf(std::path::PathBuf),
    /// 同梱の既定の写しを作る
    CopyOfBuiltIn,
}

/// 書式を直すときの書き先。
///
/// 手引き「配られたテンプレートは書き替わりません」の決めそのものです。
/// 書き先はいつも開いている文書の隣で、元がそこに無ければ写しを作ります。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// 書くファイル
    pub at: std::path::PathBuf,
    /// その中身の元
    pub origin: Origin,
}

impl Target {
    /// 写しを作るか
    pub fn copies(&self) -> bool {
        self.origin != Origin::InPlace
    }
}

/// 書き先を決める。
///
/// - `template_name` は本文の頭の `:template: 名前`。あればファイル名は
///   `名前.toml`、無ければ [`user_template_name`](`テンプレート.toml`)
/// - `tmpl_path` はいま着ているテンプレートを読んだ場所。`None` は
///   この機械の標準か同梱の既定
/// - `user_template` はこの機械の標準のファイル(あるときだけ渡す)。
///   同梱の既定より先に写しの元になります
pub fn write_target(
    doc_dir: &std::path::Path,
    template_name: Option<&str>,
    tmpl_path: Option<&std::path::Path>,
    user_template: Option<&std::path::Path>,
) -> Target {
    let file = match template_name {
        Some(n) => format!("{n}.toml"),
        None => user_template_name().to_string(),
    };
    let at = doc_dir.join(file);
    let origin = match tmpl_path {
        Some(p) if p == at => Origin::InPlace,
        Some(p) => Origin::CopyOf(p.to_path_buf()),
        None => match user_template {
            Some(u) => Origin::CopyOf(u.to_path_buf()),
            None => Origin::CopyOfBuiltIn,
        },
    };
    Target { at, origin }
}

/// 書き先の元の字を読み、`f` で直した字を書き先に書く。
///
/// 写しを作るとき、書き先に既にファイルがあれば書きません。そこにある
/// 物は読めなかったテンプレートのはずで、黙って潰すと直す手がかりが
/// 消えるためです。返りは、書いた後の字です。
pub fn rewrite(t: &Target, f: impl FnOnce(&str) -> String) -> Result<String, String> {
    let base = match &t.origin {
        Origin::InPlace => std::fs::read_to_string(&t.at).map_err(|e| e.to_string())?,
        Origin::CopyOf(p) => {
            if t.at.exists() {
                return Err(format!("{} は既にあるのに読めていません", t.at.display()));
            }
            std::fs::read_to_string(p).map_err(|e| e.to_string())?
        }
        Origin::CopyOfBuiltIn => {
            if t.at.exists() {
                return Err(format!("{} は既にあるのに読めていません", t.at.display()));
            }
            DEFAULT_TOML.to_string()
        }
    };
    let fresh = f(&base);
    // 書く前に読めることを確かめます。読めない字を書くと、次に開いたとき
    // 「テンプレートが読めない」になります
    parse(&fresh)?;
    std::fs::write(&t.at, &fresh).map_err(|e| e.to_string())?;
    Ok(fresh)
}

/// 書き出し先ごとのテンプレートのファイル名(`テンプレート-印刷.toml`)。
pub fn purpose_template_name(purpose: &str) -> String {
    format!("テンプレート-{purpose}.toml")
}

/// 利用者の標準テンプレート。無ければ同梱の既定。
///
/// **壊れていても落ちません。** 手で書く物なので、書き方を間違えたときに
/// アプリが開かなくなるのは困ります。そのときは既定に落ちます。
pub fn user_theme(config_dir: &std::path::Path) -> Theme {
    // **段ごとに畳んでから重ねます。** 自分のテンプレートが `[文書.en]`
    // でしか大きさを言っていなくても、英語の画面ならそれが効きます
    let default_of = default_theme().for_current_language();
    match read_theme(&config_dir.join(user_template_name())) {
        Some(th) => merge(th.for_current_language(), default_of),
        None => default_of,
    }
}

/// 2つのテンプレートを重ねる。**上が言っていないことだけ下から取ります**。
pub fn merge(mut top: Theme, below: Theme) -> Theme {
    if top.font.is_none() {
        top.font = below.font;
    }
    if top.size_pt.is_none() {
        top.size_pt = below.size_pt;
    }
    if top.page.is_none() {
        top.page = below.page;
    }
    if top.header.is_none() {
        top.header = below.header;
    }
    if top.footer.is_none() {
        top.footer = below.footer;
    }
    if top.watermark.is_none() {
        top.watermark = below.watermark;
    }
    // 言語ごとの分も**札で重ねます**。上に同じ札があればそちらが勝ち、
    // 上が書体だけ言っているなら大きさは下から取ります
    for (tag, d) in below.lang_docs {
        match top.lang_docs.iter_mut().find(|(t, _)| *t == tag) {
            Some((_, u)) => {
                if u.font.is_none() {
                    u.font = d.font;
                }
                if u.size_pt.is_none() {
                    u.size_pt = d.size_pt;
                }
            }
            None => top.lang_docs.push((tag, d)),
        }
    }
    // スタイルは**名前で重ねます**。上に同じ名前があればそちらが勝ちます
    for d in below.styles {
        if !top.styles.iter().any(|x| x.name == d.name) {
            top.styles.push(d);
        }
    }
    top
}

/// TOML(部分集合)からテンプレートを読む。
/// 知らない節・知らないキーは**黙って捨てずに** Err で言う — テンプレートは
/// 人と AI が書く物で、綴りの間違いに黙ると「効かない」だけが残る
/// 行の後ろの覚え書き(`#` から行末まで)を落とす。囲みの中の `#` は残す。
fn strip_note(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// **1つの読み単位にまとめた行**を返します。返りは(元の行番号, 中身)。
///
/// 覚え書き(`#` から行末)を落とし、空行を捨てます。値が `[` や `{` で
/// 開いたまま行が終わったら、閉じるまで次の行を繋ぎます — 様式のセルは
/// 配列で書くので、1つの値が何行にもまたがります(2026-08-18)。
fn logical_lines(src: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut cont: Option<(usize, String)> = None;
    for (ln, raw) in src.lines().enumerate() {
        let line = strip_note(raw).trim();
        if line.is_empty() && cont.is_none() {
            continue;
        }
        match &mut cont {
            Some((_, s)) => {
                s.push(' ');
                s.push_str(line);
            }
            None => cont = Some((ln, line.to_string())),
        }
        let (begin_at, s) = cont.as_ref().expect("いま入れた");
        if balances(s) {
            out.push((*begin_at, s.clone()));
            cont = None;
        }
    }
    if let Some(x) = cont {
        out.push(x); // 閉じていない — 値を読むところで指摘が出ます
    }
    out
}

/// `[` `{` と `]` `}` の数が釣り合っているか(囲みの中の字は数えません)
fn balances(s: &str) -> bool {
    let mut depth = 0i32;
    let mut boxed = false;
    for c in s.chars() {
        match c {
            '"' => boxed = !boxed,
            '[' | '{' if !boxed => depth += 1,
            ']' | '}' if !boxed => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}

/// **日本語のキーを `"…"` で囲んでから**ライブラリに渡します。
///
/// TOML の決まりでは、囲まないキーに使えるのは英数字と `-` `_` だけです。
/// うちのテンプレートはキーも日本語なので、そのままでは読んでもらえません
/// (2026-08-18 に実際に通して分かりました)。**書き方は変えません** —
/// 渡す直前にここで囲みます。
///
/// 直すのは `{ セル = […] }` のような値の中のキーだけです。囲みの中
/// (`"…"`)は字なので触りません。
fn quote_key(v: &str) -> String {
    let b: Vec<char> = v.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    let mut boxed = false;
    while i < b.len() {
        let c = b[i];
        if c == '"' {
            boxed = !boxed;
            out.push(c);
            i += 1;
            continue;
        }
        if boxed || !(c.is_alphanumeric() && !c.is_ascii()) {
            out.push(c);
            i += 1;
            continue;
        }
        // 日本語で始まる語。後ろが `=` ならキーなので囲む
        let mut j = i;
        while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '-') {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && b[k] == ' ' {
            k += 1;
        }
        let word: String = b[i..j].iter().collect();
        if k < b.len() && b[k] == '=' {
            out.push('"');
            out.push_str(&word);
            out.push('"');
        } else {
            out.push_str(&word);
        }
        i = j;
    }
    out
}

/// ライブラリの指摘は何行にもなるので、1行にまとめます。
/// 出すのは最初の1文だけで、場所はこちらの「N 行目」で言います
fn to_one_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

pub fn parse(src: &str) -> Result<Theme, String> {
    let mut th = Theme::default();
    // いま居る節。None = 頭(節の外)
    enum Sec {
        Submit,
        Setting,
        Doc,
        LangDoc(usize),
        Page,
        Style(usize),
        Form(usize),
    }
    let mut cur: Option<Sec> = None;
    for (ln, line) in logical_lines(src) {
        let line = line.as_str();
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let name = name.trim();
            cur = Some(if name == "送り先" || name.eq_ignore_ascii_case("submit") {
                th.submit.get_or_insert_with(Default::default);
                Sec::Submit
            } else if name == "組み方" || name.eq_ignore_ascii_case("layout") {
                Sec::Setting
            } else if name == "文書" || name.eq_ignore_ascii_case("document") {
                Sec::Doc
            } else if let Some(tag) = name
                .strip_prefix("文書.")
                .or_else(|| name.strip_prefix("document."))
            {
                // **言語ごとの [文書]**(2026-08-26)。同じ節を2回書いたら
                // 後の行が勝つよう、既にある分に足します
                let tag = tag.trim().to_string();
                let i = match th.lang_docs.iter().position(|(t, _)| *t == tag) {
                    Some(i) => i,
                    None => {
                        th.lang_docs.push((tag, LangDoc::default()));
                        th.lang_docs.len() - 1
                    }
                };
                Sec::LangDoc(i)
            } else if name == "ページ" || name.eq_ignore_ascii_case("page") {
                Sec::Page
            } else if let Some(sn) = name
                .strip_prefix("スタイル.")
                .or_else(|| name.strip_prefix("style."))
            {
                th.styles.push(StyleDef { name: sn.trim().to_string(), ..Default::default() });
                Sec::Style(th.styles.len() - 1)
            } else if let Some(fname) = name
                .strip_prefix("様式.")
                .or_else(|| name.strip_prefix("form."))
            {
                th.forms.push(Form { name: fname.trim().to_string(), rows: Vec::new() });
                Sec::Form(th.forms.len() - 1)
            } else {
                return Err(format!("{} 行目: 知らない節 [{name}](文書 / ページ / スタイル.名前)", ln + 1));
            });
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("{} 行目: 「キー = 値」の形ではありません: {line}", ln + 1));
        };
        let (k, v) = (k.trim(), v.trim());
        // **値の文法はライブラリに読ませます**(2026-08-18)。文字列の
        // 中の \" や、配列・`{ }` の入れ子まで自分で読むと、不具合の方が
        // 多くなります。節とキーの読みはこちらに残します — TOML の素の
        // キーは英数字だけと決まっていて、日本語のキーが通らないためです
        let value = |v: &str| -> Result<toml_edit::Value, String> {
            quote_key(v)
                .parse::<toml_edit::Value>()
                .map_err(|e| format!("{} 行目: 値が読めません: {}", ln + 1, to_one_line(&e.to_string())))
        };
        let s = |v: &str| -> Result<String, String> {
            value(v)?
                .as_str()
                .map(|x| x.to_string())
                .ok_or_else(|| format!("{} 行目: 文字列は \"…\" で囲みます: {v}", ln + 1))
        };
        let n = |v: &str| -> Result<f32, String> {
            let x = value(v)?;
            x.as_float()
                .map(|f| f as f32)
                .or_else(|| x.as_integer().map(|f| f as f32))
                .ok_or_else(|| format!("{} 行目: 数で書いてください: {v}", ln + 1))
        };
        let b = |v: &str| -> Result<bool, String> {
            value(v)?
                .as_bool()
                .ok_or_else(|| format!("{} 行目: true か false で書いてください: {v}", ln + 1))
        };
        match &cur {
            None => return Err(format!("{} 行目: 節の外にキーがあります: {k}", ln + 1)),
            Some(Sec::Submit) => {
                let v2 = s(v)?;
                let sub = th.submit.get_or_insert_with(Default::default);
                match k {
                    "宛先" | "action" => sub.action = v2,
                    "送り方" | "method" => {
                        let m = v2.to_ascii_lowercase();
                        if m != "post" && m != "get" {
                            return Err(format!("{} 行目: 送り方は post か get: {m}", ln + 1));
                        }
                        sub.method = m;
                    }
                    "ボタン" | "label" => sub.label = v2,
                    _ => return Err(format!("{} 行目: [送り先] の知らないキー: {k}", ln + 1)),
                }
            }
            Some(Sec::Setting) => match k {
                "横幅" | "width" => {
                    th.setting.fluid = match s(v)?.as_str() {
                        "可変" | "fluid" => true,
                        "固定" | "fixed" => false,
                        other => return Err(format!("{} 行目: 横幅は 可変 か 固定: {other}", ln + 1)),
                    }
                }
                "区切り" | "break" => {
                    th.setting.br = match s(v)?.as_str() {
                        "なし" | "none" => Break::None,
                        "ページ" | "page" => Break::Page,
                        "節" | "section" => Break::Section,
                        other => {
                            return Err(format!(
                                "{} 行目: 区切りは ページ / なし / 節: {other}",
                                ln + 1
                            ))
                        }
                    }
                }
                "跨ぎ" | "keep" => {
                    // **裏返して読む** — 「跨ぎ = false」が「跨がない」
                    th.setting.keep = !b(v)?;
                }
                _ => return Err(format!("{} 行目: [組み方] の知らないキー: {k}", ln + 1)),
            },
            Some(Sec::Doc) => match k {
                "書体" | "font" => th.font = Some(s(v)?),
                "大きさ" | "size" => th.size_pt = Some(n(v)?),
                _ => return Err(format!("{} 行目: [文書] の知らないキー: {k}", ln + 1)),
            },
            Some(Sec::LangDoc(i)) => {
                let (tag, d) = &mut th.lang_docs[*i];
                match k {
                    "書体" | "font" => d.font = Some(s(v)?),
                    "大きさ" | "size" => d.size_pt = Some(n(v)?),
                    "用紙" | "paper" => {
                        let na = s(v)?;
                        d.paper_mm = Some(
                            youshi_mm(&na)
                                .ok_or_else(|| format!("{} 行目: {}", ln + 1, youshi_error(&na)))?,
                        );
                    }
                    _ => {
                        return Err(format!(
                            "{} 行目: [文書.{tag}] の知らないキー: {k}(書体 / 大きさ / 用紙)",
                            ln + 1
                        ))
                    }
                }
            }
            Some(Sec::Page) => {
                let p = th.page.get_or_insert_with(PageSetup::default);
                match k {
                    "用紙" | "paper" => {
                        let na = s(v)?;
                        let (w, h) = youshi_mm(&na)
                            .ok_or_else(|| format!("{} 行目: {}", ln + 1, youshi_error(&na)))?;
                        (p.w_mm, p.h_mm) = (w, h);
                    }
                    "余白" | "margin" => {
                        let m = n(v)?;
                        (p.left_mm, p.right_mm, p.top_mm, p.bottom_mm) = (m, m, m, m);
                    }
                    "段組み" | "columns" => p.columns = n(v)? as u8,
                    // ページの飾り。**見た目なのでテンプレートの持ち物**
                    "ヘッダー" | "header" => th.header = Some(s(v)?),
                    "フッター" | "footer" => th.footer = Some(s(v)?),
                    "透かし" | "watermark" => th.watermark = Some(s(v)?),
                    "ページの色" | "page_color" => th.page_color = Some(s(v)?),
                    "縦書き" | "vertical" => th.vertical = b(v)?,
                    _ => return Err(format!("{} 行目: [ページ] の知らないキー: {k}", ln + 1)),
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
                    // **リボンは「段落の背景色」と呼んでいる**ので、そちらを
                    // 正しい書き方にします。「帯」は前に書いたテンプレートが
                    // 読めなくならないように受け続けます
                    "背景色" | "網かけ" | "帯" | "shade" => d.shade = Some(s(v)?),
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
                    // 1行目の字下げ。**全角の文字数で書く** — 「1字下げ」と
                    // 言うとおりに書けるのが大事で、pt や mm では言い直しになる
                    "字下げ" | "first_line" => d.first_line_chars = Some(n(v)?.max(0.0)),
                    _ => return Err(format!("{} 行目: [スタイル] の知らないキー: {k}", ln + 1)),
                }
            }
            Some(Sec::Form(i)) => {
                let name = th.forms[*i].name.clone();
                match k {
                    "行" | "rows" => th.forms[*i].rows = read_grid(&value(v)?, &name, ln)?,
                    _ => return Err(format!("{} 行目: [様式.{name}] の知らないキー: {k}", ln + 1)),
                }
            }
        }
    }
    for f in &th.forms {
        if f.rows.is_empty() {
            return Err(format!("[様式.{}] に 行 がありません", f.name));
        }
    }
    Ok(th)
}

/// 様式の `行 = [ { セル = […], 幅 = […] }, … ]` を読む。
fn read_grid(v: &toml_edit::Value, name: &str, ln: usize) -> Result<Vec<FormRow>, String> {
    let line_no = |i: usize| format!("{} 行目: [様式.{name}] の {} 行目", ln + 1, i + 1);
    let Some(array_of) = v.as_array() else {
        return Err(format!("{} 行目: [様式.{name}] の 行 は配列で書いてください", ln + 1));
    };
    let mut out = Vec::new();
    for (i, line) in array_of.iter().enumerate() {
        let Some(tbl) = line.as_inline_table() else {
            return Err(format!(
                "{}は {{ セル = [\"項目の名前\"] }} の形で書いてください",
                line_no(i)
            ));
        };
        let mut r = FormRow::default();
        for (k, v) in tbl.iter() {
            match k {
                "セル" | "cells" => {
                    let Some(a) = v.as_array() else {
                        return Err(format!("{}の セル は配列で書いてください", line_no(i)));
                    };
                    for c in a.iter() {
                        let Some(s) = c.as_str() else {
                            return Err(format!(
                                "{}の セル には項目の名前を \"…\" で書いてください",
                                line_no(i)
                            ));
                        };
                        r.cells.push(s.to_string());
                    }
                }
                "幅" | "widths" => {
                    let Some(a) = v.as_array() else {
                        return Err(format!("{}の 幅 は配列で書いてください", line_no(i)));
                    };
                    for c in a.iter() {
                        let n = c
                            .as_float()
                            .or_else(|| c.as_integer().map(|x| x as f64))
                            .ok_or_else(|| format!("{}の 幅 は数で書いてください", line_no(i)))?;
                        r.widths.push(n as f32);
                    }
                }
                _ => return Err(format!("{}の知らないキー: {k}", line_no(i))),
            }
        }
        if r.cells.is_empty() {
            return Err(format!("{}に セル がありません", line_no(i)));
        }
        out.push(r);
    }
    Ok(out)
}

/// テンプレート → TOML(正規形)。**AI と人が読み書きする物**なので、
/// キーは日本語で書き、並びは [`parse`] が読む順に揃える。
/// 門番は `parse(write(x)) == x`(往復)
pub fn write(th: &Theme) -> String {
    let mut s = String::new();
    if th.setting != Setting::default() {
        s.push_str("[組み方]\n");
        if th.setting.fluid {
            s.push_str("横幅 = \"可変\"\n");
        }
        match th.setting.br {
            Break::None => s.push_str("区切り = \"なし\"\n"),
            Break::Section => s.push_str("区切り = \"節\"\n"),
            Break::Page => {}
        }
        if th.setting.keep {
            s.push_str("跨ぎ = false\n");
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
    // **言語ごとの分も書き出します**(2026-08-26)。書かないと、保存し直した
    // ときに言語の分が消えます
    for (tag, d) in &th.lang_docs {
        if d.font.is_none() && d.size_pt.is_none() && d.paper_mm.is_none() {
            continue;
        }
        s.push_str(&format!("[文書.{tag}]\n"));
        if let Some(f) = &d.font {
            s.push_str(&format!("書体 = {f:?}\n"));
        }
        if let Some(n) = d.size_pt {
            s.push_str(&format!("大きさ = {}\n", num(n)));
        }
        if let Some(wh) = d.paper_mm {
            s.push_str(&format!("用紙 = {:?}\n", youshi_na(wh)));
        }
        s.push('\n');
    }
    // **ページの飾りだけのテンプレートもある**ので、用紙が無くても節を出す
    let has_deco = th.header.is_some()
        || th.footer.is_some()
        || th.watermark.is_some()
        || th.page_color.is_some()
        || th.vertical;
    if th.page.is_some() || has_deco {
        s.push_str("[ページ]\n");
        let default_of = crate::doc::PageSetup::default();
        let p = th.page.as_ref().unwrap_or(&default_of);
        let paper = if th.page.is_none() { None } else { match (p.w_mm, p.h_mm) {
            (297.0, 210.0) => Some("A4横"),
            (297.0, 420.0) => Some("A3"),
            (182.0, 257.0) => Some("B5"),
            (210.0, 297.0) => Some("A4"),
            _ => None,
        }};
        if let Some(k) = paper {
            s.push_str(&format!("用紙 = {k:?}\n"));
        }
        // 4辺が同じときだけ「余白」で書ける(違う値は今の器が持てない)
        if th.page.is_some()
            && p.left_mm == p.right_mm
            && p.left_mm == p.top_mm
            && p.left_mm == p.bottom_mm
        {
            s.push_str(&format!("余白 = {}\n", num(p.left_mm)));
        }
        if p.columns > 1 {
            s.push_str(&format!("段組み = {}\n", p.columns));
        }
        if let Some(h) = &th.header {
            s.push_str(&format!("ヘッダー = {h:?}\n"));
        }
        if let Some(f) = &th.footer {
            s.push_str(&format!("フッター = {f:?}\n"));
        }
        if let Some(w) = &th.watermark {
            s.push_str(&format!("透かし = {w:?}\n"));
        }
        if let Some(c) = &th.page_color {
            s.push_str(&format!("ページの色 = {c:?}\n"));
        }
        if th.vertical {
            s.push_str("縦書き = true\n");
        }
        s.push('\n');
    }
    // 様式(セル)。**行の並びが意味そのもの**なので、書いた順に出します
    for f in &th.forms {
        s.push_str(&format!("[様式.{}]\n行 = [\n", f.name));
        for r in &f.rows {
            let grid_cell: Vec<String> = r.cells.iter().map(|c| format!("\"{c}\"")).collect();
            s.push_str(&format!("  {{ セル = [{}]", grid_cell.join(", ")));
            if !r.widths.is_empty() {
                let widths: Vec<String> = r.widths.iter().map(|w| num(*w)).collect();
                s.push_str(&format!(", 幅 = [{}]", widths.join(", ")));
            }
            s.push_str(" },\n");
        }
        s.push_str("]\n\n");
    }
    for d in &th.styles {
        s.push_str(&format!("[{}]\n", style_section(&d.name)));
        for (k, v) in style_lines(d) {
            s.push_str(&format!("{k} = {v}\n"));
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
/// テンプレートに書いたヘッダー・フッターの字 → 段落。
///
/// `{ページ}` と `{ページ数}` は、その頁の数字になる印に変えます
/// (docx の PAGE / NUMPAGES と同じ物)。
fn deco_para(s: &str) -> crate::doc::Paragraph {
    let text = s
        .replace("{ページ数}", &crate::doc::PAGES_MARK.to_string())
        .replace("{ページ}", &crate::doc::PAGE_MARK.to_string());
    let mut p = crate::doc::Paragraph { line_spacing: 1.0, ..Default::default() };
    p.runs.push(crate::doc::Run {
        text,
        size_pt: None,
        font: None,
        fmt: Default::default(),
    });
    p
}

/// **文書ぜんたいに掛かる分だけ**テンプレートを合成する。
/// 書体・字の大きさ・用紙・ヘッダー・フッター・透かし・ページの色・縦書き。
///
/// 段落の書式は入れません。docx で書き出すときは、段落の見た目を
/// `styles.xml` の側が持つので、こちらだけを合成します
/// (`ooxml::write_with_theme` と対になっています)。
pub fn compose_page(out: &mut Document, theme: &Theme) {
    if out.font.is_none() {
        out.font = theme.font.clone();
    }
    if out.size_pt.is_none() {
        out.size_pt = theme.size_pt;
    }
    if out.page.is_none() {
        out.page = theme.page;
    }
    // **ページの飾りはテンプレートが持ちます**(2026-08-18)。文書が自分で
    // 持っていれば(docx から来た文書)そちらが勝ちます — 受け取った物を
    // 黙って別の見た目にしないためです
    if out.header.paragraphs.is_empty() {
        if let Some(h) = &theme.header {
            out.header.paragraphs = vec![deco_para(h)];
        }
    }
    if out.footer.paragraphs.is_empty() {
        if let Some(f) = &theme.footer {
            out.footer.paragraphs = vec![deco_para(f)];
        }
    }
    if out.watermark.is_none() {
        out.watermark = theme.watermark.clone();
    }
    if out.page_color.is_none() {
        out.page_color = theme.page_color.clone();
    }
    if theme.vertical {
        out.vertical = true;
    }
    // **桁の割合を mm に直します**(2026-08-18)。adoc は幅を比で言うので、
    // 紙の幅が決まるここで初めて mm になります。docx から読んだ表は
    // すでに mm を持っているので、割合は空で素通りします
    let line_len = out.page.map(|p| p.measure_mm()).unwrap_or(170.0);
    for b in &mut out.blocks {
        let Block::Table(t) = b else { continue };
        if t.col_ratio.is_empty() {
            continue;
        }
        let sum: f32 = t.col_ratio.iter().sum();
        if sum <= 0.0 {
            continue;
        }
        t.col_mm = t.col_ratio.iter().map(|v| v / sum * line_len).collect();
    }
}

/// **本文のラベル付きリストを、様式(セル)の表にします**(2026-08-18)。
///
/// 文書の頭に `:様式: 申請書` と書き、テンプレートに `[様式.申請書]` が
/// あるときだけ効きます。中身(`項目:: 値`)とセルの結び付けは**名前で取ります**
/// — 順番で取ると、項目を1つ足しただけで全部ずれます。
///
/// 返りは利用者に見せる言葉です。**対応の付かない項目と、埋まらないセルは
/// 必ず言います**。黙って落とすと、空欄の申請書ができあがります。
pub fn apply_forms(out: &mut Document, theme: &Theme) -> Vec<String> {
    let Some(name) = out
        .attrs
        .iter()
        .find(|(k, _)| k == "様式" || k == "form")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
    else {
        return Vec::new();
    };
    let Some(form) = theme.forms.iter().find(|f| f.name == name) else {
        return vec![format!("様式「{name}」がテンプレートにありません")];
    };

    // 本文のラベル付きリストを集める(名前 → 値)。並びは書いた順
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut src_place: Vec<usize> = Vec::new();
    for (i, b) in out.blocks.iter().enumerate() {
        let Block::Para(p) = b else { continue };
        if p.style_id.as_deref() != Some("説明のリスト") {
            continue;
        }
        let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
        if let Some((name, value)) = text.split_once(":: ") {
            entries.push((name.trim().to_string(), value.trim().to_string()));
            src_place.push(i);
        }
    }
    if src_place.is_empty() {
        return vec![format!("様式「{name}」を使う本文がありません(`項目:: 値` で書きます)")];
    }

    // セルを組む。1つのセルは「項目の名前」と「値」の2つのセットになります
    let mut used: Vec<String> = Vec::new();
    let mut says: Vec<String> = Vec::new();
    // **セルは 100 桁の格子**で組みます(2026-08-18)。
    //
    // 行ごとにセルの数が違うのが様式の普通の姿です(1行目は1つ、2行目は2つ)。
    // 表の桁は1組しか持てないので、100 の格子を敷いて、各セルが占める桁数で
    // 幅を表します。100 は百分率と同じなので、`幅 = [30, 70]` がそのまま
    // 30 桁と 70 桁になります
    const GRID: u16 = 100;
    let mut rows: Vec<Vec<crate::doc::Cellbox>> = Vec::new();
    for r in &form.rows {
        let mut content: Vec<(String, String)> = Vec::new();
        for c in &r.cells {
            let value = entries.iter().find(|(n, _)| n == c).map(|(_, v)| v.clone());
            if value.is_none() {
                says.push(format!("セル「{c}」に入れる項目が本文にありません"));
            } else {
                used.push(c.clone());
            }
            content.push((c.clone(), value.unwrap_or_default()));
        }
        // この行のセルの数(名前と値で2つずつ)
        let n = content.len() * 2;
        let widths = split_cols(&r.widths, n, GRID);
        let mut row = Vec::new();
        for (i, (name, value)) in content.iter().enumerate() {
            row.push(cell_width(name, widths[i * 2]));
            row.push(cell_width(value, widths[i * 2 + 1]));
        }
        rows.push(row);
    }
    // 格子はぜんぶ同じ幅(1%)。幅はセルの占める桁数で表しています
    let ratio: Vec<f32> = Vec::new();
    for (n, _) in &entries {
        if !used.contains(n) {
            says.push(format!("項目「{n}」に対応するセルが様式にありません"));
        }
    }

    // 表に置き換える。**ラベル付きリストは消します**(二重に出さないため)
    let insert_at = src_place[0];
    let t = crate::doc::Table { rows, col_ratio: ratio, ..Default::default() };
    for i in src_place.iter().rev() {
        out.blocks.remove(*i);
    }
    out.blocks.insert(insert_at, Block::Table(t));
    says
}

/// **幅の指定を、格子の桁数に割ります。**
///
/// `幅` が無ければ等分します。数が足りない・多いときは、書いた分だけ使って
/// 残りを等分します。合計は必ず格子の数にします(端数は最後のセルに寄せます)
fn split_cols(widths: &[f32], n: usize, grid: u16) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    let mut w: Vec<f32> = (0..n)
        .map(|i| widths.get(i).copied().filter(|x| *x > 0.0).unwrap_or(0.0))
        .collect();
    // 書いていない分は、残りを等分する
    let wrote: f32 = w.iter().sum();
    let gap = w.iter().filter(|x| **x <= 0.0).count();
    if gap > 0 {
        let rest = (grid as f32 - wrote).max(gap as f32);
        for x in w.iter_mut().filter(|x| **x <= 0.0) {
            *x = rest / gap as f32;
        }
    }
    let sum: f32 = w.iter().sum();
    let mut out: Vec<u8> = w
        .iter()
        .map(|x| ((x / sum * grid as f32).round() as i64).clamp(1, 255) as u8)
        .collect();
    // 端数で合計がずれるので、最後のセルで帳尻を合わせます
    let delta = grid as i64 - out.iter().map(|x| *x as i64).sum::<i64>();
    if let Some(last) = out.last_mut() {
        *last = (*last as i64 + delta).clamp(1, 255) as u8;
    }
    out
}

/// セル1つ(字と、占める桁数)
fn cell_width(text: &str, cols: u8) -> crate::doc::Cellbox {
    let mut c = grid_cell(text);
    c.col_span = cols;
    c
}

/// セル1つ(字を1つ入れたセル)
fn grid_cell(text: &str) -> crate::doc::Cellbox {
    crate::doc::Cellbox {
        paragraphs: vec![crate::doc::Paragraph {
            line_spacing: 1.0,
            runs: vec![crate::doc::Run {
                text: text.to_string(),
                size_pt: None,
                font: None,
                fmt: Default::default(),
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// **文書自身のスタイル定義を段落に流し込む。**
///
/// 段落が直に言っている所は触りません。docx の決めどおり、run の `w:rPr` が
/// スタイルより強く、スタイルは `docDefaults` より強い順です。
fn jibun_wo_ateru(
    para: &mut crate::doc::Paragraph,
    lk: &crate::doc::StyleLook,
    pl: &crate::doc::StyleParaLook,
) {
    // 揃えは、段落が既定(左)のままのときだけスタイルの物にします。
    // docx は「左」と「言わない」を書き分けないので、ここは見分けられません
    if para.align == crate::doc::Align::Left {
        if let Some(a) = pl.align {
            para.align = a;
        }
    }
    if para.space_before_pt == 0.0 {
        para.space_before_pt = pl.space_before_pt.unwrap_or(0.0);
    }
    if para.space_after_pt == 0.0 {
        para.space_after_pt = pl.space_after_pt.unwrap_or(0.0);
    }
    if para.line_spacing <= 0.0 && para.line_pt.is_none() {
        para.line_spacing = pl.line_spacing.unwrap_or(0.0);
    }
    if para.indent == 0 {
        para.indent = pl.indent.unwrap_or(0);
    }
    if para.first_line_twips == 0 {
        para.first_line_twips = pl.first_line_twips.unwrap_or(0);
    }
    for r in &mut para.runs {
        if r.size_pt.is_none() {
            r.size_pt = lk.size_pt;
        }
        if r.font.is_none() {
            r.font = lk.font.clone();
        }
        r.fmt.bold |= lk.bold.unwrap_or(false);
        r.fmt.italic |= lk.italic.unwrap_or(false);
        r.fmt.underline |= lk.underline.unwrap_or(false);
        if r.fmt.color.is_none() {
            r.fmt.color = lk.color.clone();
        }
    }
}

pub fn compose(doc: &Document, theme: &Theme) -> Document {
    let mut out = doc.clone();
    compose_page(&mut out, theme);
    // **文書が自分でスタイル定義を持っていれば、そちらが正です**(2026-09-01)。
    //
    // docx は `styles.xml` を一緒に持ってきます。そこにある見出しの大きさ・
    // 色・揃えを、こちらのテンプレートで上書きしていました。内閣府の
    // document_4 は見出しが 20pt・中央揃え・色付きなのに、うちの
    // テンプレートの 16pt・左揃え・黒で出ていました。
    let jibun: Vec<(String, crate::doc::StyleLook, crate::doc::StyleParaLook)> = out
        .blocks
        .iter()
        .filter_map(|b| match b {
            crate::doc::Block::Para(p) => p.style_id.clone(),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| out.style_matome(&id).map(|(l, p)| (id, l, p)))
        .collect();
    for block in &mut out.blocks {
        let crate::doc::Block::Para(para) = block else { continue };
        if let Some((_, lk, pl)) = para
            .style_id
            .as_deref()
            .and_then(|id| jibun.iter().find(|(i, _, _)| i == id))
        {
            jibun_wo_ateru(para, lk, pl);
            continue;
        }
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
            if para.line_spacing <= 0.0 {
                para.line_spacing = ls;
            }
        }
        // 1行目の字下げ。**全角の文字数 × その段落の字の大きさ**で twips に。
        // 本文が自分で持っていれば(docx 由来)そちらが勝つ
        if para.first_line_twips == 0 {
            if let Some(chars) = def.first_line_chars {
                let pt = def.size_pt.or(theme.size_pt).unwrap_or(crate::DEFAULT_PT);
                para.first_line_twips = (chars * pt * 20.0).round() as i32;
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
    // 段落だけの項目(背景色・揃え・空き・行間)は字には効かない
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
    // **1節=1枚**(発表の組み方。2026-08-17)。見出し1 の前で必ず改める。
    //
    // 印は**写しの側**に付ける — 意味の正本は「ここが節の頭だ」としか
    // 言っておらず、そこで紙を改めるかはテンプレートの決め。
    // 組んだ後に頁の境を足すのではなく**折り手に折らせる**(docx の
    // `w:pageBreakBefore` と同じ道)。後から境だけ足すと行の位置は
    // 巻物のまま動かず、境の手前の余白ぶんが前の枚に取り残される
    // (2026-08-17、実機で1枚目が見出しだけになって見つけた)
    if theme.setting.per_section() {
        for block in &mut out.blocks {
            let Block::Para(para) = block else { continue };
            if para.style == crate::doc::ParaStyle::Heading(1) {
                para.page_break_before = true;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Paragraph, Run};

    fn meaning_only_doc() -> Document {
        let h = Paragraph {
            style: ParaStyle::Heading(1),
            runs: vec![Run { text: "題".into(), size_pt: None, font: None, fmt: Default::default() }],
            ..Default::default()
        };
        let b = Paragraph {
            runs: vec![Run {
                text: "本文の字。".into(),
                size_pt: None,
                font: None,
                fmt: Default::default(),
            }],
            ..Default::default()
        };
        Document {
            blocks: vec![crate::doc::Block::Para(h), crate::doc::Block::Para(b)],
            ..Default::default()
        }
    }

    /// **様式(セル)**(2026-08-18)。中身は本文が持ち、枠だけをここに置く
    #[test]
    fn a_form_reads_and_round_trips() {
        let src = "[様式.申請書]\n行 = [\n  { セル = [\"申請日\"], 幅 = [30, 70] },\n  { セル = [\"部署\", \"氏名\"] },\n]\n";
        let th = parse(src).expect("読めない");
        assert_eq!(th.forms.len(), 1);
        assert_eq!(th.forms[0].name, "申請書");
        assert_eq!(th.forms[0].rows.len(), 2);
        assert_eq!(th.forms[0].rows[0].cells, vec!["申請日"]);
        assert_eq!(th.forms[0].rows[0].widths, vec![30.0, 70.0]);
        assert_eq!(th.forms[0].rows[1].cells, vec!["部署", "氏名"]);
        assert_eq!(write(&th), src, "往復で崩れた");
    }

    /// 値の文法はライブラリが読む。**日本語のキーは囲んでから渡す** —
    /// TOML の決まりでは、囲まないキーは英数字だけだから
    #[test]
    fn values_under_japanese_keys_read() {
        let th = parse("[様式.甲]\n行 = [{ セル = [\"あ\"] }]\n").expect("読めない");
        assert_eq!(th.forms[0].rows[0].cells, vec!["あ"]);
        // 囲みの中の `セル =` は字なので触らない
        assert_eq!(quote_key("\"セル = 1\""), "\"セル = 1\"");
    }

    fn form_doc() -> (Document, Theme) {
        let (doc, _) = crate::adoc::parse_full(
            "= 休暇申請書\n:様式: 申請書\n\n申請日:: 8月18日\n部署:: 総務課\n氏名:: 山田太郎\n",
        )
        .expect("本文が読めない");
        let th = parse(
            "[様式.申請書]\n行 = [\n  { セル = [\"申請日\"], 幅 = [30, 70] },\n  { セル = [\"部署\", \"氏名\"] },\n]\n",
        )
        .expect("様式が読めない");
        (doc, th)
    }

    #[test]
    fn a_form_binds_to_the_body_by_name() {
        let (mut doc, th) = form_doc();
        let says = apply_forms(&mut doc, &th);
        assert!(says.is_empty(), "何か言われた: {says:?}");
        let t = doc.tables().next().expect("セルにならない");
        assert_eq!(t.rows.len(), 2);
        // 1つのセルは「名前」と「値」の2つ。幅は 100 桁の格子で表す
        let text = |c: &crate::doc::Cellbox| -> String {
            c.paragraphs.iter().flat_map(|p| p.runs.iter()).map(|r| r.text.as_str()).collect()
        };
        assert_eq!(text(&t.rows[0][0]), "申請日");
        assert_eq!(text(&t.rows[0][1]), "8月18日");
        assert_eq!(t.rows[0][0].span(), 30, "幅の指定が効いていない");
        assert_eq!(t.rows[0][1].span(), 70);
        // 幅を書いていない行は等分(4つで 100)
        assert_eq!(t.rows[1].iter().map(|c| c.span()).sum::<usize>(), 100);
        // **本文は `項目:: 値` のまま残らない**(セルに置き換わる)
        assert!(
            !doc.paragraphs().any(|p| p.style_id.as_deref() == Some("説明のリスト")),
            "ラベル付きリストが二重に残った"
        );
    }

    #[test]
    fn unmatched_items_and_unfilled_cells_are_reported() {
        let (_, th) = form_doc();
        let (mut doc, _) = crate::adoc::parse_full(
            "= 題\n:様式: 申請書\n\n申請日:: 8月18日\n電話:: 03-0000-0000\n",
        )
        .unwrap();
        let says = apply_forms(&mut doc, &th);
        assert!(says.iter().any(|s| s.contains("セル「部署」")), "{says:?}");
        assert!(says.iter().any(|s| s.contains("項目「電話」")), "{says:?}");
    }

    #[test]
    fn with_no_form_name_nothing_happens() {
        let (_, th) = form_doc();
        let (mut doc, _) = crate::adoc::parse_full("= 題\n\n項目:: 値\n").unwrap();
        assert!(apply_forms(&mut doc, &th).is_empty());
        assert!(doc.tables().next().is_none(), "様式と言っていないのにセルにした");
    }

    #[test]
    fn form_errors_are_reported_in_japanese() {
        let e = parse("[様式.甲]\n行 = 1\n").unwrap_err();
        assert!(e.contains("配列で書いてください"), "{e}");
        let e = parse("[様式.甲]\n").unwrap_err();
        assert!(e.contains("行 がありません"), "{e}");
        let e = parse("[様式.甲]\n行 = [{ 枡 = [\"あ\"] }]\n").unwrap_err();
        assert!(e.contains("知らないキー"), "{e}");
    }

    #[test]
    fn the_default_theme_reproduces_the_old_direct_formatting() {
        // **段階Aの門番。** writer の set_para_style が焼き付けていた
        // 16/13/11.5pt 太字と同じ値が、合成から出ること
        let d = compose(&meaning_only_doc(), &default_theme());
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(16.0), "見出し1は16pt");
        assert!(ps[0].runs[0].fmt.bold, "見出し1は太字");
        assert_eq!(ps[1].runs[0].size_pt, None, "本文は既定のまま(焼き付けない)");
        assert!(!ps[1].runs[0].fmt.bold);
    }

    #[test]
    fn composing_leaves_the_source_document_alone() {
        let d = meaning_only_doc();
        let _ = compose(&d, &default_theme());
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, None, "意味の側は意味のまま");
    }

    #[test]
    fn direct_formatting_is_not_crushed() {
        // 互換の文書に掛けても、本文が指定した見た目が勝つ
        let mut d = meaning_only_doc();
        if let crate::doc::Block::Para(p) = &mut d.blocks[0] {
            p.runs[0].size_pt = Some(22.0);
        }
        let out = compose(&d, &default_theme());
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(22.0), "直接の 22pt が残る");
    }

    #[test]
    fn a_named_style_beats_the_role() {
        let mut th = default_theme();
        th.styles.push(StyleDef {
            name: "注意書き".into(),
            color: Some("C7433F".into()),
            ..Default::default()
        });
        let mut d = meaning_only_doc();
        if let crate::doc::Block::Para(p) = &mut d.blocks[1] {
            p.style_id = Some("注意書き".into());
        }
        let out = compose(&d, &th);
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[1].runs[0].fmt.color.as_deref(), Some("C7433F"));
    }

    #[test]
    fn a_character_style_wins_inside_a_paragraph_style() {
        let mut th = default_theme();
        // **既定のテンプレートに無い名前を使います。** 「注意」は
        // 2026-08-18 に註記の仲間として既定に入ったので、同じ名前だと
        // 既定の方が先に見つかり、この試験の意味が消えます
        th.styles.push(StyleDef {
            name: "強め".into(),
            color: Some("C7433F".into()),
            size_pt: Some(14.0),
            ..Default::default()
        });
        let mut d = meaning_only_doc();
        if let crate::doc::Block::Para(p) = &mut d.blocks[0] {
            // 見出し1(16pt)の中の1語だけ「強め」
            p.runs[0].fmt.style_id = Some("強め".into());
        }
        let out = compose(&d, &th);
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(14.0), "字の名前が段落の名前に負けた");
        assert_eq!(ps[0].runs[0].fmt.color.as_deref(), Some("C7433F"));
        assert!(ps[0].runs[0].fmt.bold, "段落の太字は残る(字の側が外していない)");
    }

    #[test]
    fn an_unknown_key_is_not_swallowed() {
        assert!(parse("[スタイル.x]\n大きき = 16\n").is_err(), "綴りの間違いに黙ると「効かない」だけが残る");
        assert!(parse("[謎の節]\n").is_err());
        assert!(parse("大きさ = 16\n").is_err(), "節の外のキー");
    }

    #[test]
    fn japanese_and_english_keys_read_the_same() {
        let ja = parse("[スタイル.見出し1]\n大きさ = 16\n太字 = true\n").unwrap();
        let en = parse("[style.見出し1]\nsize = 16\nbold = true\n").unwrap();
        assert_eq!(ja, en);
    }

    #[test]
    fn a_template_round_trips() {
        // **門番**: 書いて読むと同じ物になる(AI が書いた物も、画面が
        // 足したスタイルも、同じ表を通る)
        let src = "[文書]\n大きさ = 11\n\n[ページ]\n用紙 = \"B5\"\n余白 = 15\n\n\
                   [スタイル.見出し1]\n大きさ = 20\n太字 = true\n色 = \"1B6E3C\"\n後の空き = 8\n";
        let th = parse(src).unwrap();
        let back = write(&th);
        assert_eq!(parse(&back).unwrap(), th, "往復で崩れた:\n{back}");
    }

    #[test]
    fn layout_values_round_trip() {
        let th = parse("[組み方]\n横幅 = \"可変\"\n区切り = \"なし\"\n").unwrap();
        assert!(th.setting.fluid && th.setting.endless());
        // 発表の組み方(1節=1枚・跨がない)
        let s = parse("[組み方]\n区切り = \"節\"\n跨ぎ = false\n").unwrap();
        assert!(s.setting.per_section() && s.setting.keep);
        assert_eq!(parse(&write(&s)).unwrap(), s, "発表の組み方が往復しない");
        assert_eq!(parse(&write(&th)).unwrap(), th);
        // 既定(紙)は書かない — 空の節を増やさない
        assert_eq!(write(&Theme::default()), "");
        assert!(parse("[組み方]\n横幅 = \"なんとなく\"\n").is_err(), "知らない値に黙った");
    }

    #[test]
    fn per_language_document_sections_read_and_round_trip() {
        let th = parse(
            "[文書]\n書体 = \"IPA明朝\"\n大きさ = 10.5\n\
             [文書.en]\n書体 = \"Liberation Serif\"\n大きさ = 11\n\
             [文書.vi]\n大きさ = 13\n",
        )
        .unwrap();
        // 日本語(言語の分が無い)は [文書] のまま
        let ja = th.for_language("ja");
        assert_eq!(ja.font.as_deref(), Some("IPA明朝"));
        assert_eq!(ja.size_pt, Some(10.5));
        // 英語は書体も大きさも上書き
        let en = th.for_language("en");
        assert_eq!(en.font.as_deref(), Some("Liberation Serif"));
        assert_eq!(en.size_pt, Some(11.0));
        // ベトナム語は大きさだけ言っているので、書体は [文書] から
        let vi = th.for_language("vi");
        assert_eq!(vi.font.as_deref(), Some("IPA明朝"), "言っていない分まで上書きした");
        assert_eq!(vi.size_pt, Some(13.0));
        // 札そのものが無ければ言語だけで引く(pt-br → pt)
        let pt = parse("[文書.pt]\n大きさ = 11\n").unwrap();
        assert_eq!(pt.for_language("pt-br").size_pt, Some(11.0));
        // 書き出して読み直しても同じ
        assert_eq!(parse(&write(&th)).unwrap(), th, "往復で言語の分が消えた:\n{}", write(&th));
    }

    #[test]
    fn merging_stacks_the_per_language_parts_by_tag() {
        let top = parse("[文書.en]\n書体 = \"Georgia\"\n").unwrap();
        let below = parse("[文書.en]\n書体 = \"Arial\"\n大きさ = 11\n[文書.ko]\n大きさ = 10\n").unwrap();
        let m = merge(top, below);
        let en = m.for_language("en");
        assert_eq!(en.font.as_deref(), Some("Georgia"), "上の書体が負けた");
        assert_eq!(en.size_pt, Some(11.0), "上が言っていない大きさを下から取れていない");
        assert_eq!(m.for_language("ko").size_pt, Some(10.0), "下だけにある札が消えた");
    }

    #[test]
    fn writing_a_key_keeps_the_hand_written_lines() {
        let from = "# 自分で書いた注釈\n[文書]\n大きさ = 12\n\n[ページ]\n余白 = 20\n";
        // 節はあってキーが無い — 節の終わりに足す
        let a = put(from, "文書", "書体", "\"IPA明朝\"");
        assert!(a.contains("# 自分で書いた注釈"), "注釈が消えた:\n{a}");
        assert!(a.contains("余白 = 20"), "他の節が消えた:\n{a}");
        let th = parse(&a).unwrap();
        assert_eq!(th.font.as_deref(), Some("IPA明朝"));
        assert_eq!(th.size_pt, Some(12.0), "元からあった大きさが消えた");
        // 同じキーをもう一度 — 増やさずに書き替える
        let b = put(&a, "文書", "書体", "\"IPAexゴシック\"");
        assert_eq!(b.matches("書体 =").count(), 1, "同じキーが2行になった:\n{b}");
        assert_eq!(parse(&b).unwrap().font.as_deref(), Some("IPAexゴシック"));
        // 節ごと無い — 末尾に足す
        let c = put(from, "文書.ko", "書体", "\"NanumGothic\"");
        assert_eq!(
            parse(&c).unwrap().for_language("ko").font.as_deref(),
            Some("NanumGothic"),
        );
        // 空のファイルから
        let d = put("", "文書.ja", "書体", "\"IPA明朝\"");
        assert_eq!(parse(&d).unwrap().for_language("ja").font.as_deref(), Some("IPA明朝"));
    }

    #[test]
    fn the_page_section_reads() {
        let th = parse("[ページ]\n用紙 = \"B5\"\n余白 = 15\n").unwrap();
        let p = th.page.unwrap();
        assert_eq!((p.w_mm, p.h_mm), (182.0, 257.0));
        assert_eq!(p.left_mm, 15.0);
    }

    /// 太字を外したら `太字 = true` の行が消える。他の行は残る
    #[test]
    fn dropping_a_key_removes_only_that_line() {
        let from = "[スタイル.註記]\n# 注釈\n太字 = true\n色 = \"333333\"\n\n[スタイル.本文]\n太字 = true\n";
        let a = drop_key(from, "スタイル.註記", "太字");
        assert!(a.contains("# 注釈"), "注釈が消えた:\n{a}");
        assert!(a.contains("色 = \"333333\""));
        assert_eq!(a.matches("太字 = true").count(), 1, "別の節の行まで消えた:\n{a}");
        assert!(!parse(&a).unwrap().style("註記").unwrap().bold);
        // 無いキーを消しても字は変わらない
        assert_eq!(drop_key(from, "スタイル.註記", "斜体"), from);
    }

    /// スタイル1つを書き入れる。**持っていない項目は行ごと消える**
    #[test]
    fn putting_a_style_writes_and_clears_its_keys() {
        let from = "# 上の注釈\n[文書]\n大きさ = 11\n\n[スタイル.見出し1]\n大きさ = 16\n太字 = true\n";
        let def = StyleDef {
            name: "見出し1".into(),
            size_pt: Some(18.0),
            align: Some(Align::Center),
            ..Default::default()
        };
        let a = put_style(from, &def);
        assert!(a.contains("# 上の注釈"), "注釈が消えた:\n{a}");
        let th = parse(&a).unwrap();
        let d = th.style("見出し1").unwrap();
        assert_eq!(d.size_pt, Some(18.0));
        assert_eq!(d.align, Some(Align::Center));
        assert!(!d.bold, "外した太字が残った:\n{a}");
        assert_eq!(th.size_pt, Some(11.0), "他の節が消えた");
        // 無い節は末尾に足す
        let b = put_style(from, &StyleDef { name: "註記".into(), italic: true, ..Default::default() });
        assert!(parse(&b).unwrap().style("註記").unwrap().italic, "{b}");
        // 何も持たないスタイルでも節は残る
        let c = put_style("", &StyleDef { name: "空".into(), ..Default::default() });
        assert!(parse(&c).unwrap().style("空").is_some(), "{c}");
        // write と同じ表を見ている(往復)
        let th2 = parse(&write(&th)).unwrap();
        assert_eq!(th2.style("見出し1"), th.style("見出し1"));
    }

    #[test]
    fn renaming_a_section_keeps_its_lines() {
        let from = "[スタイル.見た目1]\n大きさ = 14\n\n[スタイル.見た目2]\n太字 = true\n";
        let a = rename_section(from, "スタイル.見た目1", "スタイル.小見出し");
        let th = parse(&a).unwrap();
        assert_eq!(th.style("小見出し").unwrap().size_pt, Some(14.0));
        assert!(th.style("見た目1").is_none());
        assert!(th.style("見た目2").unwrap().bold, "別の節が変わった");
        assert_eq!(rename_section(from, "スタイル.無い", "スタイル.何か"), from);
    }

    /// 書き先の決め。**配られた物は書き替えず、文書の隣に写しを作る**
    #[test]
    fn write_target_follows_the_handout_rule() {
        let dir = std::path::Path::new("/綴り");
        let here = dir.join("テンプレート.toml");
        // 隣の物を着ている — その場で書く
        let t = write_target(dir, None, Some(&here), None);
        assert_eq!(t, Target { at: here.clone(), origin: Origin::InPlace });
        assert!(!t.copies());
        // 名指しで置き場の物を着ている — 隣に同じ名前の写し
        let far = std::path::Path::new("/home/x/.config/officework/templates/社内標準.toml");
        let t = write_target(dir, Some("社内標準"), Some(far), None);
        assert_eq!(t.at, dir.join("社内標準.toml"));
        assert_eq!(t.origin, Origin::CopyOf(far.to_path_buf()));
        // 何も着ていない — この機械の標準があればそれ、無ければ同梱の既定
        let user = std::path::Path::new("/home/x/.config/officework/テンプレート.toml");
        let t = write_target(dir, None, None, Some(user));
        assert_eq!(t.origin, Origin::CopyOf(user.to_path_buf()));
        let t = write_target(dir, None, None, None);
        assert_eq!(t.origin, Origin::CopyOfBuiltIn);
        assert_eq!(t.at, here);
    }

    /// 写しを作って直す。**元は変わらず、写しは読める字になっている**
    #[test]
    fn rewrite_copies_then_edits_and_leaves_the_original() {
        let tmp = std::env::temp_dir().join(format!("theme-rewrite-{}", std::process::id()));
        let handout_dir = tmp.join("配布");
        let doc_dir = tmp.join("綴り");
        std::fs::create_dir_all(&handout_dir).unwrap();
        std::fs::create_dir_all(&doc_dir).unwrap();
        let handout = handout_dir.join("社内標準.toml");
        let original = "# 配られた物\n[スタイル.本文]\n大きさ = 10.5\n";
        std::fs::write(&handout, original).unwrap();
        let t = write_target(&doc_dir, Some("社内標準"), Some(&handout), None);
        let def = StyleDef { name: "本文".into(), size_pt: Some(12.0), ..Default::default() };
        let fresh = rewrite(&t, |src| put_style(src, &def)).unwrap();
        assert!(fresh.contains("# 配られた物"), "写しに元の注釈が無い");
        assert_eq!(std::fs::read_to_string(&handout).unwrap(), original, "配られた物が変わった");
        let th = read_theme(&t.at).expect("写しが読めない");
        assert_eq!(th.style("本文").unwrap().size_pt, Some(12.0));
        // 2回目は写しをその場で書く
        let t2 = write_target(&doc_dir, Some("社内標準"), Some(&t.at), None);
        assert_eq!(t2.origin, Origin::InPlace);
        let def2 = StyleDef { name: "本文".into(), size_pt: Some(13.0), ..Default::default() };
        rewrite(&t2, |src| put_style(src, &def2)).unwrap();
        assert_eq!(read_theme(&t.at).unwrap().style("本文").unwrap().size_pt, Some(13.0));
        // 同梱の既定からの写しも読める
        let t3 = write_target(&doc_dir, None, None, None);
        rewrite(&t3, |src| put_style(src, &def)).unwrap();
        let th3 = read_theme(&t3.at).expect("既定の写しが読めない");
        assert_eq!(th3.style("本文").unwrap().size_pt, Some(12.0));
        assert!(th3.style("見出し1").is_some(), "既定のスタイルが写っていない");
        // 写し先に読めないファイルがあれば書かない
        let broken_dir = tmp.join("壊れ");
        std::fs::create_dir_all(&broken_dir).unwrap();
        std::fs::write(broken_dir.join("テンプレート.toml"), "[知らない節]\n").unwrap();
        let t4 = write_target(&broken_dir, None, None, None);
        assert!(rewrite(&t4, |s| s.to_string()).is_err());
        assert_eq!(std::fs::read_to_string(broken_dir.join("テンプレート.toml")).unwrap(), "[知らない節]\n");
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// 蒸留した結果を書いて開き直す。**docx → adoc + toml → 同じ見た目**
    #[test]
    fn a_distilled_docx_reopens_from_adoc_and_toml() {
        use crate::doc::{Paragraph, Run};
        let mut doc = Document::default();
        let big = Run { text: "題".into(), size_pt: Some(20.0), font: None, fmt: Default::default() };
        let plain = Run { text: "本文の字。".into(), size_pt: Some(10.5), font: None, fmt: Default::default() };
        doc.blocks.push(Block::Para(Paragraph { runs: vec![big], ..Default::default() }));
        for _ in 0..3 {
            doc.blocks.push(Block::Para(Paragraph { runs: vec![plain.clone()], ..Default::default() }));
        }
        let (mut meaning, th, rep) = crate::distill::distill(&doc);
        // 本文は文書の既定(`[文書]` の大きさ)になり、題だけがスタイルになる
        assert_eq!(rep.styles, 1);
        meaning.template = Some("報告書".into());
        let tmp = std::env::temp_dir().join(format!("theme-distill-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("報告書.toml"), write(&th)).unwrap();
        std::fs::write(tmp.join("報告書.adoc"), crate::adoc::write(&meaning)).unwrap();
        // 開き直す
        let text = std::fs::read_to_string(tmp.join("報告書.adoc")).unwrap();
        let (again, _) = crate::adoc::parse_full(&text).expect("本文が読めない");
        assert_eq!(again.template.as_deref(), Some("報告書"));
        let th2 = read_theme(&tmp.join("報告書.toml")).expect("テンプレートが読めない");
        let shown = compose(&again, &th2);
        let sizes: Vec<Option<f32>> =
            shown.paragraphs().map(|p| p.runs.first().and_then(|r| r.size_pt)).collect();
        assert_eq!(sizes[0], Some(20.0), "題の大きさが戻らない");
        assert_eq!(sizes[1], None, "本文に大きさが焼き付いた");
        assert_eq!(th2.size_pt, Some(10.5), "本文の大きさが戻らない");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
