//! **文書の模型。** 段落・ラン・表・文書と、組み上がった紙面(`Sheet`)。
//!
//! UI にも PDF にも依らない。**画面も紙も、この紙面を別の面に写すだけ。**



/// 相互参照(docx の REF / PAGEREF フィールド)。**run 1つが1つの参照**で、
/// run の text は「いま見えている値」(更新で計算し直す)。
/// 編集で参照の中を割ったら、参照は普通の文字に降りる(予測できる形で壊す)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefField {
    /// 指す先(しおりの名前)
    pub name: String,
    /// true ならページ番号(PAGEREF)、false ならしおりの文字(REF)
    pub page: bool,
}

/// 注の番号の書式(docx の `w:footnotePr` / `w:endnotePr` の `w:numFmt`)。
///
/// **脚注と文末脚注で既定が違う** — Word も LibreOffice も、脚注は算用数字、
/// 文末脚注はローマ数字の小文字にする。実物(both-notes.docx)も
/// `decimal` と `lowerRoman` を明記していた。
///
/// ここ(模型)の既定は算用数字で、**docx の既定を知っているのは読み手のほう**
/// (settings.xml が黙っていたときに文末脚注をローマ数字にするのは ooxml の仕事)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteNumFmt {
    #[default]
    Decimal,
    LowerRoman,
    UpperRoman,
    LowerLetter,
    UpperLetter,
}

impl NoteNumFmt {
    /// docx の `w:numFmt w:val`。知らない書式は算用数字に落とす
    /// (**知った顔をしない** — 出ない番号より出る番号のほうがまし)
    pub fn from_docx(v: &str) -> NoteNumFmt {
        match v {
            "lowerRoman" => NoteNumFmt::LowerRoman,
            "upperRoman" => NoteNumFmt::UpperRoman,
            "lowerLetter" => NoteNumFmt::LowerLetter,
            "upperLetter" => NoteNumFmt::UpperLetter,
            _ => NoteNumFmt::Decimal,
        }
    }

    /// n(1 始まり)をこの書式の字にする
    pub fn label(self, n: usize) -> String {
        match self {
            NoteNumFmt::Decimal => n.to_string(),
            NoteNumFmt::LowerRoman => roman(n).to_lowercase(),
            NoteNumFmt::UpperRoman => roman(n),
            NoteNumFmt::LowerLetter => letter(n).to_lowercase(),
            NoteNumFmt::UpperLetter => letter(n),
        }
    }
}

/// 1 → I、4 → IV。0 以下は空にせず 1 として扱う(番号は必ず出す)
fn roman(n: usize) -> String {
    const T: &[(usize, &str)] = &[
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut n = n.max(1);
    let mut out = String::new();
    for (v, s) in T {
        while n >= *v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

/// 1 → A、27 → AA(Word の付け方)
fn letter(n: usize) -> String {
    let n = n.max(1);
    let i = (n - 1) % 26;
    let rep = (n - 1) / 26 + 1;
    std::iter::repeat_n((b'A' + i as u8) as char, rep).collect()
}

/// 脚注ひとつぶんの中身。本文の印とは `id` で繋がる。
#[derive(Debug, Clone, Default)]
pub struct Footnote {
    /// docx の `w:id`。**原文のまま**(振り直さない)
    pub id: String,
    pub endnote: bool,
    /// 脚注の文章。段落の並び(普通は1段落)
    pub paragraphs: Vec<Paragraph>,
    /// **このアプリで足した**注。保存でこちらが部品
    /// (`footnotes.xml`・宣言・関係)ごと書き出す。
    /// 読み込んだ注(原本の部品が持っている)とは持ち場が違う —
    /// 混ぜると保存で二重になる(`images` と `images_new` と同じ関係)
    pub added: bool,
}

/// 脚注・文末脚注の印(docx の `w:footnoteReference` / `w:endnoteReference`)。
///
/// **中身は持たない。** 脚注の文章は `word/footnotes.xml` にあり、
/// そこは保存で原本のまま持ち越される部品なので触らない。ここが持つのは
/// 「本文のこの位置が、その id の脚注を指している」という**印だけ**。
///
/// `id` を数ではなく字で持つのは、**書き手によって番号の付け方が違う**から
/// (pandoc は 20・21・22、LibreOffice は 2・3・4、仕切り線に -1 を使う物もある)。
/// 数に直して振り直すと、footnotes.xml 側と繋がらなくなる
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteRef {
    /// docx の `w:id`。**原文のまま**持つ(振り直さない)
    pub id: String,
    /// true なら文末脚注(endnote)、false なら脚注(footnote)
    pub endnote: bool,
}

/// 文字の書式。**docx の `w:rPr` に対応する。**
///
/// 既定(全部 false・色なし)が素の本文。`Default` で作れば何も付かない。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharFormat {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// 上付き(x²)・下付き(H₂O)。docx の w:vertAlign
    pub superscript: bool,
    pub subscript: bool,
    /// 蛍光ペン。docx の w:highlight(yellow 等の名前で持つ)
    pub highlight: Option<String>,
    /// 文字色。`RRGGBB`(docx の `w:color w:val` と同じ形)
    pub color: Option<String>,
    /// 相互参照。ここ(書式)に持つのは、run の分割・結合・描画の
    /// 既存の道具立てがそのまま使えるため(field が違えば繋がらない)
    pub field: Option<RefField>,
    /// ルビ(ふりがな)。この run の字の上に半分の大きさで振る。
    /// field と同じ理由でここに持つ — run の切り貼りが面倒を見てくれる
    pub ruby: Option<String>,
    /// 脚注・文末脚注の印。**位置が意味そのもの**なので、段落の控え
    /// (anchors)ではなく run に持つ — どの語に付いた注かが変わると困る。
    /// field・ruby と同じ持ち場で、run の切り貼りがそのまま効く。
    /// **この run は字を持たない**(印だけの run)
    pub footnote: Option<FootnoteRef>,
    /// 記入欄(docx の w:sdt = コンテンツコントロール)。
    /// **ここに持つと欄の中を普通に打てる** — 中で run が割れても、
    /// 両方が同じ欄を名乗るので欄は保たれる(field・ruby と逆で、
    /// 分割で落とさない。欄の中の編集は欄の中身だから)
    pub sdt: Option<Box<Sdt>>,
    /// リンク先(docx の `w:hyperlink` の外部の的。URL をそのまま)。
    /// **ここ(書式)に持つのは field・ruby と同じ理由** — run の切り貼りが
    /// そのまま効く。持たないと、リンクは読みも報告もされず保存で黙って
    /// 消えていた(2026-08-13 に踏んで足した)
    pub link: Option<String>,
    /// 文字スタイル(docx の `w:rStyle w:val`。原文のまま)。
    /// 定義は styles.xml が持ち、こちらは名前を運んで返すだけ
    /// (2026-08-12 発注者確定 — スタイルを捨てない)
    pub style_id: Option<String>,
}

/// 記入欄の中身(docx の w:sdtPr)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sdt {
    pub kind: SdtKind,
    /// 画面に出す名前(w:alias)
    pub alias: String,
    /// 機械で引く名前(w:tag)
    pub tag: String,
    /// 選ばせる欄の選択肢(コンボ・ドロップダウン)
    pub items: Vec<String>,
}

/// 記入欄の種類。docx の sdtPr の子要素に対応する
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SdtKind {
    #[default]
    Text,
    /// 選べる+打てる(w:comboBox)
    Combo,
    /// 選ぶだけ(w:dropDownList)
    Dropdown,
    /// ☐ / ☑(w14:checkbox)
    Checkbox,
    /// 絵(w:picture)
    Picture,
    /// 日付(w:date)
    Date,
    /// 以下はうちの区別(docx では text として往復し、tag に印を残す)
    Email,
    Phone,
    Complex,
    Signature,
}

impl SdtKind {
    /// 画面に出す呼び名
    pub fn label(self) -> &'static str {
        match self {
            SdtKind::Text => "テキスト",
            SdtKind::Combo => "コンボ",
            SdtKind::Dropdown => "ドロップダウン",
            SdtKind::Checkbox => "チェック",
            SdtKind::Picture => "画像",
            SdtKind::Date => "日付",
            SdtKind::Email => "メール",
            SdtKind::Phone => "電話",
            SdtKind::Complex => "複合",
            SdtKind::Signature => "署名",
        }
    }
    /// docx の tag に残す印(うちの区別を往復させるため)
    pub fn as_tag(self) -> &'static str {
        match self {
            SdtKind::Email => "jo:email",
            SdtKind::Phone => "jo:phone",
            SdtKind::Complex => "jo:complex",
            SdtKind::Signature => "jo:signature",
            _ => "",
        }
    }
    pub fn from_tag(tag: &str) -> Option<SdtKind> {
        match tag {
            "jo:email" => Some(SdtKind::Email),
            "jo:phone" => Some(SdtKind::Phone),
            "jo:complex" => Some(SdtKind::Complex),
            "jo:signature" => Some(SdtKind::Signature),
            _ => None,
        }
    }

    /// docx の tag から(種類, 名前)を解く。「jo:email」は種類だけ
    /// (名前は印のまま)、「jo:email:連絡先」は種類+名前 —
    /// 「名前」ボタンで付けた名とうちだけの種類の印を、一つの w:tag で両立させる形
    pub fn split_tag(tag: &str) -> Option<(SdtKind, String)> {
        use SdtKind as K;
        for k in [K::Email, K::Phone, K::Complex, K::Signature] {
            let m = k.as_tag();
            if tag == m {
                return Some((k, tag.to_string()));
            }
            if let Some(rest) = tag.strip_prefix(m).and_then(|r| r.strip_prefix(':')) {
                if !rest.is_empty() {
                    return Some((k, rest.to_string()));
                }
            }
        }
        None
    }
}

impl CharFormat {
    pub fn is_plain(&self) -> bool {
        *self == CharFormat::default()
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub size_pt: f32,
    /// 書体の名前。**フォントは文書の設定**であって、アプリの好みではない。
    /// docx の `w:rFonts`、xlsx の `<font><name>` に入っているもの。
    /// `None` は文書の既定に従う
    pub font: Option<String>,
    pub fmt: CharFormat,
}

/// 段落に入っている画像。表示のためのもの。
#[derive(Debug, Clone)]
pub struct InlineImage {
    /// 画像ファイルの中身(png/jpeg のまま)
    pub bytes: std::sync::Arc<Vec<u8>>,
    pub w_mm: f32,
    pub h_mm: f32,
}

/// 段落の揃え。docx の `w:jc`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    /// 両端揃え
    Justify,
    /// 均等割付(docx の distribute)。**最後の行も**行長いっぱいに
    /// 字間を配る — 両端揃えとの違いはそこ。見出しや表の項目名の作法
    Distribute,
}

impl Align {
    /// docx の `w:jc w:val` の値
    pub fn as_docx(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
            Align::Justify => "both",
            Align::Distribute => "distribute",
        }
    }
    pub fn from_docx(v: &str) -> Align {
        match v {
            "center" => Align::Center,
            "right" | "end" => Align::Right,
            "both" => Align::Justify,
            "distribute" => Align::Distribute,
            _ => Align::Left,
        }
    }
}

/// 箇条書きの種類。docx の `w:numPr` に対応する。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListKind {
    #[default]
    None,
    /// 中黒の箇条書き
    Bullet,
    /// 段落番号
    Number,
}

/// 段落の役割(docx の `w:pStyle` のうち、このアプリが**意味を知る**もの)。
///
/// 見出しは目次の材料。目次の行は「目次の更新」で作り直すための印。
///
/// **役割を知らないスタイルも捨てない**(2026-08-12 発注者確定 —
/// 「スタイル定義は持たない主義では無理」)。原文の styleId は
/// [`Paragraph::style_id`] が運び、定義(styles.xml)は原文を正として
/// 持ち越し、足した分だけ追記する([`Document::styles_new`])。
/// 見た目の直接書式が第一、は変わらない — スタイルは名前と定義を
/// **落とさない**ための入れ物。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParaStyle {
    #[default]
    Body,
    /// 見出し(1〜3)。docx の Heading1〜3 / outlineLvl
    Heading(u8),
    /// 目次の行(1〜3)。docx の TOC1〜3(このアプリが目次を作った印)
    Toc(u8),
    /// 図表目次の行(docx の TableofFigures。「図表目次の更新」の印)
    Tof,
}

/// 段落に付くコメント(docx の comments.xml)。**段落単位**で持つ
/// (文中の範囲は持たない — 2026-08-03 発注者確認の粒度)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comment {
    pub author: String,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    /// 段落の役割(見出し・目次の行)。既定は本文
    pub style: ParaStyle,
    /// 原文の `w:pStyle w:val`(そのまま)。役割を知らないスタイル名も
    /// **保存で消さない**ためにここが運ぶ(2026-08-12 発注者確定)。
    /// None はスタイル指定なし(このアプリで作った段落は役割から書く)
    pub style_id: Option<String>,
    /// この段落に付いたコメント
    pub comments: Vec<Comment>,
    /// この段落に付いたしおり(docx の bookmarkStart の名前)。
    /// 段落単位で持つ(範囲は段落まるごと — コメントと同じ粒度)
    pub bookmarks: Vec<String>,
    /// 読めなかった要素(画像など)の原文。**理解はしないが、捨てない。**
    /// 保存でそのまま返す
    pub anchors: Vec<String>,
    /// **この段落で終わる節**(`w:pPr` の中の `w:sectPr`)。
    /// 途中の節の区切りはここに載り、**最後の節は `Document::sect_raw`** が持つ
    /// (docx はそう書き分ける)
    pub sect: Option<SectionBreak>,
    /// 表示できる画像(anchors のうち、絵の実体と大きさが分かったもの)。
    /// 保存には使わない — 保存は anchors の原文が担う
    pub images: Vec<InlineImage>,
    /// **このアプリで挿した**画像。保存でこちらが部品(media・rels)ごと書き出す。
    /// 読み込んだ画像(anchors 由来)とは持ち場が違う — 混ぜると保存で二重になる
    pub images_new: Vec<InlineImage>,
    pub align: Align,
    /// この段落の前で改ページする(docx の w:pageBreakBefore)
    pub page_break_before: bool,
    pub list: ListKind,
    /// 左のインデント段数。1段 = 全角2文字ぶん(日本の書類の慣習)
    pub indent: u8,
    /// 1行目の字下げ(twip。正= w:firstLine、負= w:hanging のぶら下げ)。
    /// **原文の値をそのまま持って往復する** — 段落を触っても落とさないための箱で、
    /// 紙面はまだ使わない(組みに効かせるのは K4 の均等割付と同じ回で)
    pub first_line_twips: i32,
    /// 行間の倍率。1.0 が既定
    pub line_spacing: f32,
    /// 段落の背景色 `RRGGBB`(docx の w:shd)。見出しの帯に使われる
    pub shade: Option<String>,
    /// 段落を枠で囲む(docx の w:pBdr)。囲みの注意書きに使われる
    pub boxed: bool,
    /// ドロップキャップ(頭の1字を大きく)。docx では w:framePr の
    /// 「枠の段落+本文の段落」に割れるが、モデルでは1つの段落で持つ
    pub dropcap: bool,
}

impl Paragraph {
    /// 行間の倍率。0 や負が入っていても壊れない値を返す。
    pub fn spacing(&self) -> f32 {
        if self.line_spacing <= 0.0 { 1.0 } else { self.line_spacing.clamp(0.5, 5.0) }
    }

    /// 箇条書きの頭に付く印。組版のときに本文の前へ置く。
    /// **レベル(インデント)で印が変わる**(Word の複数レベルのリストの慣習)。
    pub fn marker(&self, nth: usize) -> Option<String> {
        match self.list {
            ListKind::None => None,
            ListKind::Bullet => Some(
                match self.indent % 3 {
                    0 => "・",
                    1 => "○",
                    _ => "■",
                }
                .into(),
            ),
            ListKind::Number => Some(match self.indent {
                0 => format!("{}. ", nth + 1),
                1 => format!("({}) ", nth + 1),
                _ => format!("{}) ", nth + 1),
            }),
        }
    }
}

/// 縦のセル結合(docx の w:vMerge)。
/// **日本の様式は結合で見出しを作る**ので、読めないと枠がずれて出る。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VMerge {
    #[default]
    None,
    /// 結合の始まり(w:vMerge w:val="restart")。下の Continue を呑み込む
    Start,
    /// 結合の続き。上のセルに呑まれており、中身は描かれない
    Continue,
}

/// 表の1セル。中は段落の列(セルの中にも段落がある)
#[derive(Debug, Clone, Default)]
pub struct Cellbox {
    pub paragraphs: Vec<Paragraph>,
    /// 横の結合(docx の w:gridSpan)。このセルが占める格子の列数。
    /// 0 と 1 はどちらも「結合なし」(既定の 0 を特別扱いしない)
    pub col_span: u8,
    /// 縦の結合
    pub v_merge: VMerge,
}

impl Cellbox {
    /// 占める格子の列数(最低1)。
    pub fn span(&self) -> usize {
        (self.col_span as usize).max(1)
    }
}

/// 罫線の表。日本の事務様式の本体。
#[derive(Debug, Clone, Default)]
pub struct Table {
    pub rows: Vec<Vec<Cellbox>>,
    /// 列の幅(mm)。docx の `w:gridCol`。空なら等分
    pub col_mm: Vec<f32>,
    /// 表のスタイルの**名前だけ**(docx の `w:tblStyle w:val`)。
    /// 定義(styles.xml)は持たない主義のまま — 名前を運んで返すだけ。
    /// 読めた名前を書きで落とすと様式が崩れるので、往復のために持つ
    pub style: Option<String>,
    /// 表の置き方(docx の tblPr の `w:jc`)。None は指定なし(左)。
    /// 使うのは Left / Center / Right だけ(表の置き方に両端揃えは無い)
    pub align: Option<Align>,
    /// 列幅を固定する(docx の `w:tblLayout w:type="fixed"`)。
    /// **裏返しで持つ** — docx の既定は autofit(要素なし)なので、
    /// `Default` の false がそのまま「autofit」になる
    pub fixed_layout: bool,
}

/// 文書の中身は、段落か表。順序を保つ。
#[derive(Debug, Clone)]
pub enum Block {
    Para(Paragraph),
    Table(Table),
}

/// 用紙の設定(mm)。docx の `w:pgSz` / `w:pgMar` / `w:cols`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    pub w_mm: f32,
    pub h_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
    pub top_mm: f32,
    pub bottom_mm: f32,
    /// 段組みの段数(docx の w:cols w:num)。1 が普通の1段
    pub columns: u8,
}

impl Default for PageSetup {
    fn default() -> Self {
        // A4 縦・余白 20mm(日本の事務の慣行に近い値)
        PageSetup { w_mm: 210.0, h_mm: 297.0, left_mm: 20.0, right_mm: 20.0,
                    top_mm: 20.0, bottom_mm: 20.0, columns: 1 }
    }
}

/// 節の切れ目。**この段落でひとつの節が終わる**という印。
///
/// 3つを**まとめて1つにしてある**。別々の欄にすると、片方だけ更新して
/// 食い違う形の事故が必ず出る(必ず一緒に動く物なので)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SectionBreak {
    /// 保存でそのまま返す原文。**理解はしないが捨てない**の口
    /// (`anchors` と同じ持ち場)
    pub raw: String,
    /// 組版のための顔 — 行長と紙の高さを決める。
    /// **engine は docx を解析しない**ので、読み手が解いた形で受け取る
    pub page: PageSetup,
    /// `w:type="continuous"` — **改ページしない**節。
    /// 段組みを変えるためだけの節が実物には多く、そこで頁を割ると
    /// 見た目が大きく変わる(2026-08-10、pyoffice の指摘)
    pub continuous: bool,
}

/// 段の間(mm)。Word の既定(425twip ≒ 7.5mm)に合わせる
pub const COLUMN_GAP_MM: f32 = 7.5;

impl PageSetup {
    /// 行長(本文の幅)
    pub fn measure_mm(&self) -> f32 {
        (self.w_mm - self.left_mm - self.right_mm).max(10.0)
    }

    /// 段数(0 が入っていても壊れない)
    pub fn cols(&self) -> usize {
        (self.columns as usize).clamp(1, 8)
    }

    /// 1段の行長。段組みなら段の間を除いて割る
    pub fn column_measure_mm(&self) -> f32 {
        let n = self.cols() as f32;
        ((self.measure_mm() - COLUMN_GAP_MM * (n - 1.0)) / n).max(10.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Document {
    /// 本文の流れ(段落と表が混ざる)
    pub blocks: Vec<Block>,
    /// 文書の既定の書体(docx の `w:docDefaults`)。
    /// 段落側が指定していなければこれを使う
    pub font: Option<String>,
    /// 用紙の設定。無ければ既定(A4)
    pub page: Option<PageSetup>,
    /// 節の設定の原文(w:sectPr)。ヘッダーの参照などが入っているので、
    /// **理解はしないが捨てない**。保存でそのまま返す
    pub sect_raw: Option<String>,
    /// 脚注・文末脚注の**中身**(docx の `word/footnotes.xml` / `endnotes.xml`)。
    /// 本文側の印([`CharFormat::footnote`])と **id で繋がる**。
    ///
    /// **保存はここを見ない** — 部品は原本のまま持ち越されるので、
    /// ここは**紙面に出すためだけ**に読む(番号を振り、下の領域に組む)。
    /// 仕切り線の定義(`w:type="separator"` など)は本物の脚注ではないので入れない
    pub footnotes: Vec<Footnote>,
    /// **部品に既にある注の id**(種類つき)。仕切り線の定義も含む。
    /// 仕切りは `footnotes` に載せない — 本物の注ではないので — が、
    /// **id は取られている**。新しい注に番号を選ぶとき、ここを見ないとぶつかる
    pub note_ids_taken: Vec<(String, bool)>,
    /// 脚注の番号の書式(docx の `w:footnotePr/w:numFmt`)
    pub footnote_fmt: NoteNumFmt,
    /// 文末脚注の番号の書式(`w:endnotePr/w:numFmt`)。**脚注とは別**
    pub endnote_fmt: NoteNumFmt,
    /// ヘッダー(docx の headerN.xml)。全ページ同じもの(type="default")だけを持つ
    pub header: HeadFoot,
    /// フッター(docx の footerN.xml)
    pub footer: HeadFoot,
    /// ページの色 `RRGGBB`(docx の w:background)。画面も紙も同じ色に塗る
    pub page_color: Option<String>,
    /// 透かし(斜めの薄い字)。docx ではヘッダーの中の VML の図形になる
    pub watermark: Option<String>,
    /// 手描きの線(描画タブのペン)。docx では自由曲線の図形になる
    pub ink: Vec<Stroke>,
    /// 変更履歴の書き手(w:ins / w:del の author)。
    /// 保存用の写しにだけ入る — 印([`TRK_INS_S`] 等)と対で使う
    pub track_author: Option<String>,
    /// 欧文のハイフネーション(docx の settings の autoHyphenation)。
    /// 日本語には掛からない(禁則で折る)。英語の語を音節で折って - を付ける
    pub hyphenate: bool,
    /// 文書の保護(docx の settings の documentProtection の w:edit)。
    /// Some("readOnly") なら読み取り専用。パスワード無しの保護は Word と
    /// 同じく「注意書き」— 解除のボタンで誰でも外せる(そう見せる)
    pub protection: Option<String>,
    /// 文書の情報(docx の docProps/core.xml)。作成者・タイトルなど
    pub props: CoreProps,
    /// スタイル定義の**名乗りの一覧**(styles.xml から読んだ id・名前・種類)。
    /// 定義の本体は原文の styles.xml が持ち、保存で丸ごと持ち越される —
    /// ここは「どんなスタイルがあるか」を見せる写し(2026-08-12 発注者確定)
    pub styles: Vec<StyleInfo>,
    /// このアプリで**足した**スタイル。保存で styles.xml の末尾に追記する
    /// (core.xml と同じ「原文へ差す」外科術 — 作り直さない)
    pub styles_new: Vec<StyleInfo>,
    /// 縦書き(docx の sectPr の textDirection=tbRl)。
    /// 組みは fold_vertical が行を右からの列へ写す(K4)
    pub vertical: bool,
}

/// スタイルの名乗り(styles.xml の w:style の id・名前・種類)。
/// kind は docx の type のまま: "paragraph" / "character" / "table" / "numbering"
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// 文書の情報(core properties)。空の欄は書かない
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoreProps {
    pub creator: String,
    pub title: String,
    pub keywords: String,
    pub subject: String,
    pub description: String,
}

/// 変更履歴の印。保存用の写しの本文に埋め、docx の w:ins / w:del になる。
/// PAGE の印と同じ作法(私用領域の字。画面には出さない)。
pub const TRK_INS_S: char = '\u{E010}';
/// 挿入の終わり
pub const TRK_INS_E: char = '\u{E011}';
/// 削除の始まり(中の字は w:delText — 消された字として残る)
pub const TRK_DEL_S: char = '\u{E012}';
/// 削除の終わり
pub const TRK_DEL_E: char = '\u{E013}';

/// 手描きの1筆。座標は**そのページの中**の mm(紙の左上が原点)。
/// ページに貼り付く(本文を編集しても動かない)— Word のページ固定の図形と同じ。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stroke {
    /// 何ページ目か(0始まり)
    pub page: usize,
    /// 蛍光ペンか(太く・薄く・文字の下に描く)
    pub highlighter: bool,
    /// 筆の通り道(mm)
    pub points: Vec<(f32, f32)>,
}

/// ヘッダー・フッター(1節ぶん)。
///
/// **paragraphs が空 = 持っていない(または編集できない)。**
/// 空のヘッダーを表すときは、空のランを持つ段落を1つ置く —
/// この区別で「触っていない部品を保存で書き潰さない」を守る。
#[derive(Debug, Clone, Default)]
pub struct HeadFoot {
    pub paragraphs: Vec<Paragraph>,
    /// 読み込んだ docx での部品名(`word/header1.xml` 等)。
    /// 保存で同じ部品へ書き戻す。None で paragraphs があれば新しい部品を作る
    pub part: Option<String>,
}

/// ページ番号の印(docx の PAGE フィールド)。ヘッダー・フッターの文中に
/// この1字で置き、組むとき([`layout_hf`])にそのページの番号の字になる。
/// 私用領域の字なので普通の本文と衝突しない。
pub const PAGE_MARK: char = '\u{E000}';

/// ページ数(総頁)の印(docx の NUMPAGES フィールド)。扱いは [`PAGE_MARK`] と同じ。
pub const PAGES_MARK: char = '\u{E001}';

/// 段落の列を編集用の平文にする(区切りは改行)。セル・ヘッダーの編集で使う。
pub fn paras_text(paras: &[Paragraph]) -> String {
    paras
        .iter()
        .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 平文を段落の列へ戻す。同じ位置の段落から書式を引き継ぐ
/// (本文の `set_body_text` と同じ規則 — 段落をまるごと写す)。
pub fn set_paras_text(paras: &mut Vec<Paragraph>, text: &str, size_pt: f32) {
    let old = std::mem::take(paras);
    *paras = text
        .split('\n')
        .enumerate()
        .map(|(i, s)| {
            let mut p = old.get(i).cloned().unwrap_or_default();
            let (pt, font, fmt) = p
                .runs
                .first()
                .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
                .unwrap_or((size_pt, None, CharFormat::default()));
            p.runs = vec![Run { text: s.to_string(), size_pt: pt, font, fmt }];
            p
        })
        .collect();
}

impl Document {
    /// 段落だけを順に見る(組版は v0 では段落のみを組む)
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Para(p) => Some(p),
            Block::Table(_) => None,
        })
    }
    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Table(t) => Some(t),
            Block::Para(_) => None,
        })
    }
    pub fn push_para(&mut self, p: Paragraph) {
        self.blocks.push(Block::Para(p));
    }

    /// 本文を編集用のプレーンテキストにする(表は含めない)。
    /// 段落の区切りは改行。編集はこの形で行い、保存時に段落へ戻す。
    pub fn body_text(&self) -> String {
        self.paragraphs()
            .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 編集後のテキストを本文へ写す。
    ///
    /// **1回の編集は、連続した1箇所の置き換え**(打鍵・削除・貼り付け・
    /// IME の確定・undo の1手、すべてそう)。だから頭と尻尾の共通部分を
    /// 除けば、その1箇所が分かる。そこだけを run の構造に写すので、
    /// **run の境(部分書式)・段落の性質・表の位置が編集で流されない**
    /// (以前の「段落番号で写す」方式は、段落の増減で下の性質がずれ、
    /// 表が末尾へ動いていた)。
    pub fn set_body_text(&mut self, text: &str, size_pt: f32) {
        let old = self.body_text();
        if old == text {
            return;
        }
        let (ob, nb) = (old.as_bytes(), text.as_bytes());
        // 共通の頭(文字の境に合わせる)
        let mut pre = ob.iter().zip(nb).take_while(|(a, b)| a == b).count();
        while pre > 0 && !(old.is_char_boundary(pre) && text.is_char_boundary(pre)) {
            pre -= 1;
        }
        // 共通の尻尾(頭と重ならない範囲で)
        let max_suf = (ob.len() - pre).min(nb.len() - pre);
        let mut suf = ob
            .iter()
            .rev()
            .zip(nb.iter().rev())
            .take_while(|(a, b)| a == b)
            .count()
            .min(max_suf);
        while suf > 0
            && !(old.is_char_boundary(ob.len() - suf) && text.is_char_boundary(nb.len() - suf))
        {
            suf -= 1;
        }
        self.splice_text(pre, ob.len() - suf, &text[pre..nb.len() - suf], size_pt);
    }

    /// 本文の `start..end`(バイト。段落は \n で繋いだ物差し)を `insert` で
    /// 置き換える。run の境と段落の性質を保つ、編集モデルの心臓。
    pub fn splice_text(&mut self, start: usize, end: usize, insert: &str, size_pt: f32) {
        // 段落ごとの(blocks の位置, 本文での頭, 長さ)
        let mut paras: Vec<(usize, usize, usize)> = Vec::new();
        let mut at = 0usize;
        for (bi, b) in self.blocks.iter().enumerate() {
            if let Block::Para(p) = b {
                let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                paras.push((bi, at, len));
                at += len + 1;
            }
        }
        if paras.is_empty() {
            self.blocks.push(Block::Para(Paragraph {
                line_spacing: 1.0,
                runs: vec![Run { text: insert.to_string(), size_pt, font: None,
                                 fmt: CharFormat::default() }],
                ..Default::default()
            }));
            return;
        }
        // 置き換えの端が入る段落。段落の末尾(= \n の位置)も、その段落に数える
        let pi_of = |pos: usize| -> usize {
            paras
                .iter()
                .rposition(|(_, s, _)| *s <= pos)
                .unwrap_or(0)
        };
        let (ps, pe) = (pi_of(start), pi_of(end));
        let (bi_s, s0, _) = paras[ps];
        let (bi_e, e0, e_len) = paras[pe];
        let off_s = (start - s0).min(paras[ps].2);
        let off_e = (end - e0).min(e_len);

        let head_para = match &self.blocks[bi_s] {
            Block::Para(p) => p.clone(),
            _ => unreachable!(),
        };
        let tail_para = match &self.blocks[bi_e] {
            Block::Para(p) => p.clone(),
            _ => unreachable!(),
        };
        let (head_runs, _) = split_runs(&head_para.runs, off_s);
        let (_, tail_runs) = split_runs(&tail_para.runs, off_e);
        // 差し込む字の書式は、置き換えの直前の字のもの(無ければ直後、無ければ既定)
        let (ins_pt, ins_font, ins_fmt) = head_runs
            .iter()
            .rev()
            .find(|r| !r.text.is_empty())
            .or_else(|| head_para.runs.first())
            .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
            .unwrap_or((size_pt, None, CharFormat::default()));
        let ins_run = |t: &str| {
            let mut fmt = ins_fmt.clone();
            // 参照(フィールド)の直後に打った字は参照の一部ではない。
            // ルビも同じ(打った字に読みは付いてこない)
            fmt.field = None;
            fmt.ruby = None;
            Run { text: t.to_string(), size_pt: ins_pt, font: ins_font.clone(), fmt }
        };

        // 新しい段落の列を組み立てる
        let segs: Vec<&str> = insert.split('\n').collect();
        let mut out: Vec<Paragraph> = Vec::new();
        if segs.len() == 1 {
            // 段落の増減なし(または段落の合流)。head + 差し込み + tail が1つに
            let mut p = head_para.clone();
            p.runs = head_runs;
            p.runs.push(ins_run(segs[0]));
            p.runs.extend(tail_runs);
            // 合流したら、消えた側の性質は頭の段落に呑まれる
            out.push(p);
        } else {
            let mut first = head_para.clone();
            first.runs = head_runs;
            first.runs.push(ins_run(segs[0]));
            out.push(first);
            for seg in &segs[1..segs.len() - 1] {
                let mut mid = head_para.clone();
                mid.anchors = Vec::new();
                mid.images = Vec::new();
                mid.images_new = Vec::new();
                mid.comments = Vec::new();
                mid.bookmarks = Vec::new();
                mid.runs = vec![ins_run(seg)];
                out.push(mid);
            }
            // 最後の段落は、置き換えの尻の段落の性質を継ぐ(Enter で割った
            // 後ろ半分が、元の段落の性質のまま残る形)
            let mut last = tail_para.clone();
            last.runs = vec![ins_run(segs[segs.len() - 1])];
            last.runs.extend(tail_runs);
            if ps == pe {
                // 1つの段落を割ったときは、控え(画像など)は前半にだけ残す
                last.anchors = Vec::new();
                last.images = Vec::new();
                last.images_new = Vec::new();
                last.comments = Vec::new();
                last.bookmarks = Vec::new();
            }
            out.push(last);
        }
        for p in &mut out {
            normalize_runs(&mut p.runs, size_pt);
        }
        // 置き換えの範囲に挟まっていた表などは、後ろへ避けて残す(消さない)
        let kept: Vec<Block> = self.blocks[bi_s..=bi_e]
            .iter()
            .filter(|b| !matches!(b, Block::Para(_)))
            .cloned()
            .collect();
        let mut new_blocks: Vec<Block> = out.into_iter().map(Block::Para).collect();
        new_blocks.extend(kept);
        self.blocks.splice(bi_s..=bi_e, new_blocks);
    }

    /// 平文の中の位置(バイト)から、何番目の段落かを出す。
    fn para_range(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        let mut at = 0usize;
        let (mut first, mut last) = (usize::MAX, 0usize);
        for (i, p) in self.paragraphs().enumerate() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            let end = at + len;
            // 空の選択(カーソルだけ)でも、その段落は対象にする
            if range.start <= end && range.end >= at {
                first = first.min(i);
                last = last.max(i);
            }
            at = end + 1; // 改行1つぶん
        }
        if first == usize::MAX { 0..0 } else { first..last + 1 }
    }

    /// 選択範囲の run に手を入れる(部分書式の心臓)。
    /// 範囲の境で run を割り、中の run にだけ f を掛ける。
    /// 端が字の途中に掛かっていたら、その字はまるごと含める。
    /// 選んだ範囲を**脚注にする**。その字を注の本文へ移し、跡に
    /// **印だけの run**(字を持たない)を残す。返すのは置いた印。
    ///
    /// **段落をまたぐ範囲は受けない**(None を返す)。注は文の中の一点に付く
    /// ものなので、またいだ範囲をどう畳むかに正解が無い —
    /// 決められないことを黙って決めない。
    ///
    /// 番号は組むときに出てくる順で振るので、ここでは決めない。
    pub fn make_footnote(
        &mut self,
        range: std::ops::Range<usize>,
        endnote: bool,
    ) -> Option<FootnoteRef> {
        let body = self.body_text();
        let mut start = range.start.min(body.len());
        while start > 0 && !body.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = range.end.min(body.len());
        while end < body.len() && !body.is_char_boundary(end) {
            end += 1;
        }
        if start >= end {
            return None;
        }
        // どの段落に入っているか。またいでいたら受けない
        let mut at = 0usize;
        let mut hit: Option<(usize, usize, usize)> = None; // (ブロック番号, 段落内 start, end)
        for (bi, b) in self.blocks.iter().enumerate() {
            let Block::Para(p) = b else { continue };
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            let (ps, pe) = (at, at + len);
            at = pe + 1;
            if start >= ps && end <= pe {
                hit = Some((bi, start - ps, end - ps));
                break;
            }
            if start < pe && end > pe {
                return None; // 段落をまたいでいる
            }
        }
        let (bi, ls, le) = hit?;
        let Some(Block::Para(p)) = self.blocks.get_mut(bi) else { return None };

        let (left, rest) = split_runs(&p.runs, ls);
        let (mid, right) = split_runs(&rest, le - ls);
        if mid.iter().all(|r| r.text.is_empty()) {
            return None; // 字が無い(印だけを注にはできない)
        }
        let pt = p.runs.first().map(|r| r.size_pt).unwrap_or(10.5);

        // 移した字が注の本文になる。段落は1つ
        let note_para = Paragraph {
            runs: mid,
            line_spacing: 1.0,
            ..Default::default()
        };
        let fr = self.add_footnote(endnote, vec![note_para]);

        // 跡に印だけを置く
        let Some(Block::Para(p)) = self.blocks.get_mut(bi) else { return None };
        let mut runs = left;
        runs.push(Run {
            text: String::new(),
            size_pt: pt,
            font: None,
            fmt: CharFormat {
                footnote: Some(fr.clone()),
                ..Default::default()
            },
        });
        runs.extend(right);
        p.runs = runs;
        normalize_runs(&mut p.runs, pt);
        Some(fr)
    }

    fn apply_runs(&mut self, range: std::ops::Range<usize>, f: impl Fn(&mut Run)) {
        let body = self.body_text();
        let mut start = range.start.min(body.len());
        while start > 0 && !body.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = range.end.min(body.len());
        while end < body.len() && !body.is_char_boundary(end) {
            end += 1;
        }
        let mut at = 0usize;
        for b in &mut self.blocks {
            let Block::Para(p) = b else { continue };
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            let (ps, pe) = (at, at + len);
            at = pe + 1;
            if pe < start || ps > end {
                continue;
            }
            let ls = start.saturating_sub(ps).min(len);
            let le = end.saturating_sub(ps).min(len);
            if ls >= le {
                continue;
            }
            let (left, rest) = split_runs(&p.runs, ls);
            let (mut mid, right) = split_runs(&rest, le - ls);
            for r in &mut mid {
                f(r);
            }
            let pt = p.runs.first().map(|r| r.size_pt).unwrap_or(10.5);
            p.runs = left;
            p.runs.extend(mid);
            p.runs.extend(right);
            normalize_runs(&mut p.runs, pt);
        }
    }

    /// 選択範囲の文字書式を変える。**選択の字にだけ掛かる**(run を割る)。
    /// 選択が空(カーソルだけ)のときは、その段落まるごと — 従来の作法。
    pub fn apply_char_format(
        &mut self,
        range: std::ops::Range<usize>,
        f: impl Fn(&mut CharFormat),
    ) {
        if range.start < range.end {
            self.apply_runs(range, |r| f(&mut r.fmt));
            return;
        }
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if !target.contains(&i) {
                continue;
            }
            if let Block::Para(p) = b {
                for r in &mut p.runs {
                    f(&mut r.fmt);
                }
            }
        }
    }

    /// 選択範囲の文字の大きさを変える(選択の字にだけ。空なら段落まるごと)。
    ///
    /// 上限と下限を持つ — **際限なく大きく/小さくできると事故になる**
    /// (0pt にすると本文が消え、原因が分からなくなる)。
    pub fn apply_size(&mut self, range: std::ops::Range<usize>, f: impl Fn(f32) -> f32) {
        if range.start < range.end {
            self.apply_runs(range, |r| r.size_pt = f(r.size_pt).clamp(4.0, 400.0));
            return;
        }
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if !target.contains(&i) {
                continue;
            }
            if let Block::Para(p) = b {
                for r in &mut p.runs {
                    r.size_pt = f(r.size_pt).clamp(4.0, 400.0);
                }
            }
        }
    }

    /// 選択範囲の書体を変える(選択の字にだけ。空なら段落まるごと)。
    pub fn apply_font(&mut self, range: std::ops::Range<usize>, name: Option<String>) {
        if range.start < range.end {
            self.apply_runs(range, |r| r.font = name.clone());
            return;
        }
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if !target.contains(&i) {
                continue;
            }
            if let Block::Para(p) = b {
                for r in &mut p.runs {
                    r.font = name.clone();
                }
            }
        }
    }

    /// 範囲に相互参照を付ける(挿した値の run を参照にする)・外す。
    pub fn apply_field(&mut self, range: std::ops::Range<usize>, field: Option<RefField>) {
        self.apply_runs(range, |r| r.fmt.field = field.clone());
    }

    /// 本文の中の参照を数え、値を計算し直す。`value(名前, ページか)` が
    /// 新しい値を返す(計算できなければ None = 触らない)。
    /// 返り値は書き換えた数。**run の text を直に書き換える**ので、
    /// 編集中の平文を持つ側(writer)は呼んだ後に同期し直すこと。
    pub fn refresh_fields(
        &mut self,
        value: impl Fn(&str, bool) -> Option<String>,
    ) -> usize {
        let mut n = 0usize;
        for b in &mut self.blocks {
            let Block::Para(p) = b else { continue };
            for r in &mut p.runs {
                let Some(f) = &r.fmt.field else { continue };
                if let Some(v) = value(&f.name, f.page) {
                    if v != r.text {
                        r.text = v;
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// 選択範囲(の頭)の位置にある run。書式の表示・読み取りの元。
    fn run_at(&self, pos: usize) -> Option<&Run> {
        let mut at = 0usize;
        for p in self.paragraphs() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            if pos <= at + len {
                let off = pos - at;
                // カーソルの**直前の字**の run(打つとその書式になる、の慣習)。
                // 段落の頭では最初の run
                let mut racc = 0usize;
                for r in &p.runs {
                    let rend = racc + r.text.len();
                    if off <= rend && (off > racc || racc == 0) {
                        return Some(r);
                    }
                    racc = rend;
                }
                return p.runs.first();
            }
            at += len + 1;
        }
        None
    }

    /// いま選択範囲の文字の大きさ。
    pub fn size_at(&self, range: std::ops::Range<usize>) -> Option<f32> {
        self.run_at(range.start).map(|r| r.size_pt)
    }

    /// 選択範囲にかかる段落の性質(箇条書き・インデント・行間)を変える。
    pub fn apply_para(&mut self, range: std::ops::Range<usize>, f: impl Fn(&mut Paragraph)) {
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if target.contains(&i) {
                if let Block::Para(p) = b {
                    f(p);
                }
            }
        }
    }

    /// いま選択範囲の段落の性質。
    pub fn para_at(&self, range: std::ops::Range<usize>) -> Option<&Paragraph> {
        let target = self.para_range(range);
        self.paragraphs().nth(target.start)
    }

    /// 選択範囲にかかる段落の揃えを変える。
    pub fn apply_align(&mut self, range: std::ops::Range<usize>, align: Align) {
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if target.contains(&i) {
                if let Block::Para(p) = b {
                    p.align = align;
                }
            }
        }
    }

    /// いま選択範囲が太字か(ボタンを押した状態に見せるため)。
    /// カーソルの位置の run の書式を返す。
    pub fn char_format_at(&self, range: std::ops::Range<usize>) -> CharFormat {
        self.run_at(range.start).map(|r| r.fmt.clone()).unwrap_or_default()
    }

    /// いま選択範囲の揃え。
    pub fn align_at(&self, range: std::ops::Range<usize>) -> Align {
        let target = self.para_range(range);
        self.paragraphs().nth(target.start).map(|p| p.align).unwrap_or_default()
    }

    /// 読み込み後の整え: 空 run を除き、同じ書式の隣り合う run を繋ぐ
    /// (本文・表のセル・ヘッダー・フッターの全段落)。
    /// Word の編集は同じ書式でも run を細切れにする(校正・rsid)。
    /// モデルを軽く保つのが主目的で、雛形の「{{差し込み口}}」が
    /// 道具の目に割れて見える事故の保険にもなる(docxtpl 0.20 は多くの
    /// 分断を自力で繋ぐと実測した — が、賭けにはしない)。
    /// 書式の違う分断は繋がない(書式は据え置きの方針どおり)
    pub fn heal_runs(&mut self) {
        fn heal(p: &mut Paragraph) {
            let pt = p.runs.first().map(|r| r.size_pt).unwrap_or(10.5);
            normalize_runs(&mut p.runs, pt);
        }
        for b in &mut self.blocks {
            match b {
                Block::Para(p) => heal(p),
                Block::Table(t) => {
                    for row in &mut t.rows {
                        for c in row {
                            c.paragraphs.iter_mut().for_each(heal);
                        }
                    }
                }
            }
        }
        self.header.paragraphs.iter_mut().for_each(heal);
        self.footer.paragraphs.iter_mut().for_each(heal);
    }

    /// カーソル位置の記入欄(sdt)が本文のどこからどこまでかを返す。
    /// 太字などで run が割れていても、同じ欄が続く限り一つに繋げる。
    /// 欄でない場所なら None。名前の付け替え(欄まるごと)に使う
    pub fn sdt_range_at(&self, pos: usize) -> Option<std::ops::Range<usize>> {
        let mut at = 0usize;
        for p in self.paragraphs() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            if pos <= at + len {
                // 段の中の run を(始まり, 終わり, 欄)の列に開く
                let mut spans = Vec::new();
                let mut s = at;
                for r in &p.runs {
                    spans.push((s, s + r.text.len(), r.fmt.sdt.as_deref()));
                    s += r.text.len();
                }
                // 直前の字の run(run_at と同じ慣習)。段落の頭では最初の run
                let i = spans
                    .iter()
                    .position(|&(s0, e0, _)| pos <= e0 && (pos > s0 || s0 == at))?;
                let want = spans[i].2?;
                let (mut s0, mut e0) = (spans[i].0, spans[i].1);
                for &(s1, _, sd) in spans[..i].iter().rev() {
                    if sd == Some(want) {
                        s0 = s1;
                    } else {
                        break;
                    }
                }
                for &(_, e1, sd) in &spans[i + 1..] {
                    if sd == Some(want) {
                        e0 = e1;
                    } else {
                        break;
                    }
                }
                return Some(s0..e0);
            }
            at += len + 1;
        }
        None
    }
}

/// run の列を byte で二つに割る(境に掛かる run はそこで切る)。
pub(super) fn split_runs(runs: &[Run], byte: usize) -> (Vec<Run>, Vec<Run>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut at = 0usize;
    for r in runs {
        let end = at + r.text.len();
        if end <= byte {
            left.push(r.clone());
        } else if at >= byte {
            right.push(r.clone());
        } else {
            let cut = byte - at;
            let mut a = r.clone();
            a.text = r.text[..cut].to_string();
            let mut b = r.clone();
            b.text = r.text[cut..].to_string();
            // 参照(フィールド)の中を割った = 手で書き換えた。
            // 半分ずつが参照を名乗ると更新で二重になるので、普通の字に降ろす。
            // ルビも同じ(半分の基底に同じ読みが二重に付くのを防ぐ)
            a.fmt.field = None;
            b.fmt.field = None;
            a.fmt.ruby = None;
            b.fmt.ruby = None;
            left.push(a);
            right.push(b);
        }
        at = end;
    }
    (left, right)
}

/// 空の run を除き、同じ書式の隣り合う run を繋ぐ(編集で際限なく増やさない)。
/// 全部空なら、空の run を1つ残す(空の段落の形)。
pub(super) fn normalize_runs(runs: &mut Vec<Run>, size_pt: f32) {
    let mut out: Vec<Run> = Vec::new();
    for r in runs.drain(..) {
        // **脚注の印だけは、字が無くても残す。** ここで落とすと、
        // 読めていても編集や組版を一度通っただけで印が消える
        if r.text.is_empty() && r.fmt.footnote.is_none() {
            continue;
        }
        match out.last_mut() {
            Some(last)
                if last.size_pt == r.size_pt && last.font == r.font && last.fmt == r.fmt =>
            {
                last.text.push_str(&r.text);
            }
            _ => out.push(r),
        }
    }
    if out.is_empty() {
        out.push(Run { text: String::new(), size_pt, font: None,
                       fmt: CharFormat::default() });
    }
    *runs = out;
}

impl Document {
    /// 脚注(`endnote` が真なら文末脚注)を足し、本文に置く**印**を返す。
    ///
    /// **id は既にある物と衝突しない値を選ぶ。** docx は
    /// `footnotes.xml` と `endnotes.xml` を別々に番号付けするので、
    /// 衝突を見るのは**同じ種類の中だけ**でよい。
    /// 番号(1・2・i・ii)は組むときに出てくる順で振るので、ここでは決めない。
    ///
    /// 返した印を run の `fmt.footnote` に入れれば、その位置が注の位置になる。
    pub fn add_footnote(&mut self, endnote: bool, paragraphs: Vec<Paragraph>) -> FootnoteRef {
        let mut n = 1u32;
        while self.footnotes.iter().any(|f| f.endnote == endnote && f.id == n.to_string())
            || self
                .note_ids_taken
                .iter()
                .any(|(id, e)| *e == endnote && *id == n.to_string())
        {
            n += 1;
        }
        let id = n.to_string();
        self.footnotes.push(Footnote {
            id: id.clone(),
            endnote,
            paragraphs,
            added: true,
        });
        FootnoteRef { id, endnote }
    }

    pub fn plain(text: &str, size_pt: f32) -> Document {
        Document { note_ids_taken: Vec::new(), footnote_fmt: Default::default(), endnote_fmt: Default::default(),
            font: None,
            page: None,
            sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            styles: Vec::new(), styles_new: Vec::new(),
            blocks: text
                .split('\n')
                .map(|p| Block::Para(Paragraph {
                    line_spacing: 1.0,
                    shade: None, boxed: false, images_new: Vec::new(), runs: vec![Run { text: p.to_string(), size_pt, font: None, fmt: Default::default() }],
                    ..Default::default() }))
                .collect(),
        }
    }
}

// ---------- 紙面(組んだ結果) ----------

/// 置かれた1文字。座標は紙の左上原点・mm。
#[derive(Debug, Clone)]
pub struct Cell {
    pub ch: char,
    pub x_mm: f32,
    pub w_mm: f32,
    pub size_pt: f32,
    /// 段落の頭からのバイト位置。**カーソルはこの値で本文と結ぶ**
    /// (行の文字数で数えると、折り返しや落とした空白でずれる)
    pub off: usize,
    /// この字の書式。**画面も紙も同じものを見る**ので、
    /// 太字や色が片方だけ出ることが起きない
    pub fmt: CharFormat,
    /// この字の書体(run の指定。None は文書の既定)。
    /// 行の中で書体が混ざっても、描く側が連なりごとに切り替えられる
    pub font: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub cells: Vec<Cell>,
    pub y_mm: f32, // ベースライン
    /// 本文由来か。**表のセルの行は false** —
    /// カーソルや変換下線の位置合わせは本文の行だけを数える
    pub from_body: bool,
    /// この行の頭が、本文(段落を \n で繋いだもの)の何バイト目か。
    /// 表の行では**セルの文章**の中の位置
    pub byte0: usize,
    /// 表のセル由来なら (表の番号, 行, 列)
    pub cell: Option<(usize, usize, usize)>,
}

impl Line {
    /// この行が本文の何バイト目までを含むか(行末の改行は含まない)。
    ///
    /// 行の中の字は連続しているとは限らない(折り返しで空白が落ちる)ので、
    /// 最後の字の段落内位置から出す。
    pub fn byte_end(&self) -> usize {
        let base = self.cells.iter().map(|c| c.off).min().unwrap_or(0);
        self.cells
            .last()
            .map(|c| self.byte0 + (c.off + c.ch.len_utf8()) - base)
            .unwrap_or(self.byte0)
    }
}

impl Line {
    pub fn text(&self) -> String {
        self.cells.iter().map(|c| c.ch).collect()
    }
    pub fn width_mm(&self) -> f32 {
        self.cells.iter().map(|c| c.w_mm).sum()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sheet {
    pub lines: Vec<Line>,
    /// 縦書きか。真なら vert_x が行ごとの「列の左肩の x(絶対 mm)」を持ち、
    /// Cell.x_mm は「上からの距離」の意味になる。描く側は1字ずつ置く
    pub vertical: bool,
    pub vert_x: Vec<f32>,
    /// ここで新しいページを始める、という y(巻物の座標)。
    /// 紙に写す側([`paper`]相当)がこれを見て強制的に頁を割る
    pub breaks: Vec<f32>,
    /// 引く線(表の罫線)。[x1, y1, x2, y2] mm。
    /// 画面も紙も、これをそのまま引く
    pub rules: Vec<[f32; 4]>,
    /// 表のセルの当たり判定(クリックでセルを選ぶため)
    pub cell_boxes: Vec<CellBox>,
    /// 置いた画像(実体, [x, 上端y, 幅, 高さ] mm)。画面も紙もこれを見る
    pub images: Vec<(std::sync::Arc<Vec<u8>>, [f32; 4])>,
    /// **紙面の下に出す脚注**。組み上がった行を、印のある本文の行の y と
    /// 一緒に持つ。どのページに載るかは折る側([`paper`] の頁割り)が決める —
    /// **脚注の高さは本文に使える高さを削る**ので、頁割りと切り離せない
    pub notes: Vec<NoteBlock>,
    /// **節ごとの用紙**。「この y(巻物の座標)から先はこの用紙」の並びで、
    /// y の昇順。節が1つだけの文書では**空**にしてある — 空なら今までどおり
    /// 呼ぶ側の用紙1つで折ればよい、という約束(既存の道を変えないため)。
    /// 節の切れ目は必ず [`Sheet::breaks`] にも入るので、折る側は
    /// 「頁が変わった所で用紙を引き直す」だけでよい
    pub sect_pages: Vec<(f32, PageSetup)>,
}

impl Sheet {
    /// 巻物の高さ `y` に効いている用紙。`sect_pages` が空なら None
    /// (節が1つ = 呼ぶ側の用紙をそのまま使う)。
    pub fn setup_at(&self, y: f32) -> Option<PageSetup> {
        let mut cur = None;
        for (at, pg) in &self.sect_pages {
            if y >= *at - 0.01 {
                cur = Some(*pg);
            } else {
                break;
            }
        }
        cur
    }
}

/// 紙面の下に出す脚注ひとつぶん(組み上がった形)。
#[derive(Debug, Clone, Default)]
pub struct NoteBlock {
    /// 出てくる順の番号(本文の印と同じ数)
    pub no: usize,
    /// **印のある本文の行の y**(巻物)。この行が載るページの下に出す
    pub at_y: f32,
    /// 組み上がった行。`y_mm` は**この脚注の中の相対**(0 から下へ)
    pub lines: Vec<Line>,
    /// 高さ(mm)
    pub h_mm: f32,
}

/// 表のセル1つぶんの場所。
#[derive(Debug, Clone, Copy)]
pub struct CellBox {
    pub table: usize,
    pub row: usize,
    pub col: usize,
    pub x_mm: f32,
    pub top_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
}
