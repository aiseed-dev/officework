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

/// **タブの既定の刻み(twip)。** docx の `w:defaultTabStop` の既定は 720
/// (= 0.5インチ)です。`w:pPr/w:tabs` がどれも越えていないときに使います。
pub const TAB_TWIPS: i32 = 720;

/// **書式を「言った」かどうか。** [`CharFormat::itta`] が持ちます。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Itta {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl Itta {
    /// 1つでも言っているか
    pub fn nanika(&self) -> bool {
        self.bold || self.italic || self.underline || self.strike
    }
}

/// 文字の書式。**docx の `w:rPr` に対応する。**
///
/// 既定(全部 false・色なし)が普通の本文。`Default` で作れば何も付きません。
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
    /// **字間(pt)。** docx の `w:rPr` の `w:spacing`(1/20 pt)です。
    ///
    /// 1文字ごとに、この幅だけ余分に送ります。負なら詰めます。
    /// 日本語の Word はこれで行末を揃えるので、読まないと**1行に入る
    /// 文字数が変わります**。内閣府の調査票は run ごとに 0.35〜0.85pt を
    /// 持っていて、うちは1行あたり2文字ほど多く詰めていました
    /// (2026-09-01 発注者「漢字の横幅は同じはず。どうして文字数が違ってくる」)。
    pub spacing_pt: f32,
    /// **どの書式を「言った」か。**
    ///
    /// docx の `<w:b/>` は入、`<w:b w:val="0"/>` は切、要素そのものが
    /// 無ければ「言わない」の三択です。上の `bold` などは入か切かしか
    /// 持てないので、言ったかどうかをここに持ちます。
    ///
    /// 言わない書式はスタイルや文書の既定から受け継ぎます。二択に潰すと
    /// 受け継ぎの情報が消え、`w:val="0"`(あえて切る)も書き戻せません
    /// (2026-09-01、python-docx との差分検査で分かりました)。
    pub itta: Itta,
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
    /// ラジオボタン。docx には Word のチェック(w14:checkbox)として書き、
    /// tag の `jo:radio` で種類を残します(Word にラジオの部品は無い)
    Radio,
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
            SdtKind::Radio => "ラジオ",
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
            SdtKind::Radio => "jo:radio",
            _ => "",
        }
    }
    pub fn from_tag(tag: &str) -> Option<SdtKind> {
        match tag {
            "jo:email" => Some(SdtKind::Email),
            "jo:phone" => Some(SdtKind::Phone),
            "jo:complex" => Some(SdtKind::Complex),
            "jo:signature" => Some(SdtKind::Signature),
            "jo:radio" => Some(SdtKind::Radio),
            _ => None,
        }
    }

    /// docx の tag から(種類, 名前)を解く。「jo:email」は種類だけ
    /// (名前は印のまま)、「jo:email:連絡先」は種類+名前 —
    /// 「名前」ボタンで付けた名とうちだけの種類の印を、一つの w:tag で両立させる形
    pub fn split_tag(tag: &str) -> Option<(SdtKind, String)> {
        use SdtKind as K;
        for k in [K::Email, K::Phone, K::Complex, K::Signature, K::Radio] {
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

/// 文書が大きさを何も言っていないときの見た目(pt)。**模型には入れない** —
/// 模型では「言っていない」は `None` のまま持ち、画面・紙に写す瞬間だけ
/// この値で解く。ここを模型に焼き込むと、無指定の docx が往復で
/// 「10.5pt 指定」に化ける(2026-08-13、本家 python-docx との突き合わせで発覚)
pub const DEFAULT_PT: f32 = 10.5;

#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    /// 字の大きさ(pt)。docx の `w:sz`。
    /// `None` は文書の既定に従う — `font` と同じ意味論。
    /// **表示で困るからと勝手に数を入れない**(それが焼き付きの正体だった)
    pub size_pt: Option<f32>,
    /// 書体の名前。**フォントは文書の設定**であって、アプリの好みではない。
    /// docx の `w:rFonts`、xlsx の `<font><name>` に入っているもの。
    /// `None` は文書の既定に従う
    pub font: Option<String>,
    pub fmt: CharFormat,
}

impl Run {
    /// 画面・紙に写すときの大きさ。`base` は文書の既定
    /// ([`Document::base_pt`])。模型の外へ出る瞬間だけここで解く
    pub fn pt(&self, base: f32) -> f32 {
        self.size_pt.unwrap_or(base)
    }
}

/// 段落に入っている画像。表示のためのもの。
#[derive(Debug, Clone)]
pub struct InlineImage {
    /// 画像ファイルの中身(png/jpeg のまま)
    pub bytes: std::sync::Arc<Vec<u8>>,
    pub w_mm: f32,
    pub h_mm: f32,
    /// **数式なら、その原文(LaTeX)。** 絵は組んだ結果でしかないので、
    /// これが無いと開き直したとき直せない(絵を消して打ち直しになる)。
    /// docx では画像の代替テキスト(`wp:docPr descr`)に積んで往復する —
    /// 渡した先の Word では絵として見え、こちらでは式として直せる。
    /// 普通の画像は None
    pub tex: Option<String>,
    /// ネイティブ文書(.adoc)での**相対の径路**(`image::images/図1.png[]`)。
    /// 画像の実体はファイルが正本で、bytes は開いたときの写し。
    /// docx 由来の画像は None(bytes が正本)
    pub src: Option<String>,
    /// **段落の字の中でこの画像が居る位置**(先頭からのバイト数)。
    ///
    /// docx の `<wp:inline>` は run の中に入るので、字の流れの一部です。
    /// 位置を持たないと段落の下にしか置けず、内閣府の document_4 では
    /// 見出しの絵が見出しの1行下に落ちていました(2026-09-01)。
    /// いまは段落の頭(0)にある物だけを行の中に置きます。
    pub off: usize,
}

/// **段落の罫線の、引く辺**(docx の `w:pBdr` の子)。
///
/// `between` は「同じ指定の段落が続くとき、その間に引く」です。
/// 上下の辺と合わせて、記入欄の並びが1本ずつ線で仕切られます。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParaBorder {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
    pub between: bool,
    /// **字と線の間の空き(pt)。** docx の `w:space`(pt そのもの)。
    /// 線の上と下の両方に空きます
    pub space_pt: f32,
    /// **線の太さ(pt)。** docx の `w:sz`(1/8 pt)
    pub w_pt: f32,
}

impl ParaBorder {
    /// 1辺でも引くか
    pub fn aru(&self) -> bool {
        self.top || self.bottom || self.left || self.right || self.between
    }

    /// 4辺を囲む(画面の「囲み」の切り替えが作る形)
    pub fn kakomi() -> Self {
        ParaBorder { top: true, bottom: true, left: true, right: true, ..Default::default() }
    }

    /// **線が段落に足す高さ(pt)。** 線の上と下の空きと、線の太さです。
    /// 線を引かない段落は 0 です
    pub fn takasa_pt(&self) -> f32 {
        if self.bottom || self.between || self.top {
            self.space_pt * 2.0 + self.w_pt
        } else {
            0.0
        }
    }
}

/// 段落の揃え。docx の `w:jc`。
///
/// `Hash` は蒸留([`crate::distill`])が使う — 見た目の鍵にして
/// 同じ見た目の段落をまとめる
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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
    /// **文書の表題**(AsciiDoc の `= 題`、docx の Title)。
    ///
    /// 2026-08-18 まで表題は [`CoreProps::title`] にしか入らず、**紙面には
    /// 出ませんでした**(開くと題名が消えて見える)。本文の段落にしたので、
    /// 画面で直せて、紙にも Web にも出ます。`props.title` にも同じ字が入り、
    /// docx の文書の情報として往復します
    Title,
    /// 見出し(1〜5)。docx の Heading1〜5 / outlineLvl。
    /// AsciiDoc では `==`(1段)から `======`(5段)まで
    Heading(u8),
    /// 目次の行(1〜3)。docx の TOC1〜3(このアプリが目次を作った印)
    Toc(u8),
    /// 図表目次の行(docx の TableofFigures。「図表目次の更新」の印)
    Tof,
    /// 引用(2026-08-16、AsciiDoc の ____ を受けるために足した。
    /// docx では w:pStyle "Quote")
    Quote,
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
    /// **文書が決めている箇条書きの印**(docx の `numbering.xml` の
    /// `w:lvlText`)。`○` や `(%1)` のような書き方で、`%1` はその段の番号に
    /// 置き替わります。
    ///
    /// `None` なら段の深さから作ります([`Paragraph::marker`])。前は常に
    /// そちらで、内閣府の調査票が9か所で使っている `○` が中黒で出て
    /// いました(2026-08-31)。
    pub list_text: Option<String>,
    /// 左のインデント段数。1段 = 全角2文字ぶん(日本の書類の慣習)。
    ///
    /// **段数なので、1文字や3文字は表せません。** 箇条書きの深さでもあり
    /// (adoc の `*` の数・HTML の入れ子)、刻みは変えられません。docx から
    /// 読んだ細かい値は [`left_twips`](Self::left_twips) が持ちます
    pub indent: u8,
    /// 左のインデント(twip)。docx の `w:ind w:left` をそのまま持ちます。
    ///
    /// **段数では足りないので足しました**(2026-08-30)。内閣府の告知書で、
    /// 1文字(210 twip)の字下げが2文字に、3文字が4文字になっていました。
    /// 段数は 420 twip きざみに丸めるためです。
    ///
    /// 0 は「指定なし」で、そのときは `indent` の段数を使います。
    pub left_twips: i32,
    /// **タブの止まる位置**(docx の `w:pPr/w:tabs`。twip、左からの距離)。
    ///
    /// 行にタブ(`\t`)が来たら、この一覧の中でいまの位置より右にある
    /// いちばん近い所まで送ります。どれも越えていれば、既定の刻み
    /// ([`crate::TAB_TWIPS`])で送ります。
    ///
    /// 読まないとタブが1文字ぶんにしかならず、内閣府の調査票では
    /// 氏名欄の下線が 78.6pt ぶん縮んでいました(2026-09-01)。
    pub tab_stops: Vec<i32>,
    /// **寄せを言ったかどうか。** docx は `w:jc` が無ければ「言わない」で、
    /// スタイルから受け継ぎます。`align` の既定は左なので、言わない場合と
    /// 「左と言った」場合が見分けられません。ここで見分けます(2026-09-01)。
    pub align_itta: bool,
    /// **1行目の字下げの、文字数での指定**(docx の `w:firstLineChars` /
    /// `w:hangingChars`。100 = 1文字)。日本語の Word はこちらでよく書きます。
    ///
    /// `first_line_twips` は Word がその段落の字の大きさで解いた値です。
    /// 字の大きさが 12pt なら 100文字ぶんは 240twip、10.5pt なら 210twip に
    /// なるので、**組むときは文字数から解き直します**(2026-09-01)。
    /// python-docx は twip の方を返すので、読む口はそちらを渡します。
    pub first_line_chars: Option<f32>,
    /// 1行目の字下げ(twip。正= w:firstLine、負= w:hanging のぶら下げ)。
    /// **原文の値をそのまま持って往復する** — 段落を触っても落とさないための箱で、
    /// 紙面はまだ使わない(組みに効かせるのは K4 の均等割付と同じ回で)
    pub first_line_twips: i32,
    /// 行間の倍率。1.0 が既定。`line_pt` が入っているときは見ません
    pub line_spacing: f32,
    /// **行の高さそのもの(pt)。** docx の `w:lineRule` が `exact` か
    /// `atLeast` のときに入ります。`exact` はこの高さで固定、`atLeast` は
    /// 下限です(`true` が exact)。
    ///
    /// 前は [`crate::LINE_MM`] で割って倍率に直していました。行の高さが
    /// 6.4mm の決め打ちだったので割れましたが、書体から出すようにすると
    /// 割れません(2026-09-01)。
    pub line_pt: Option<(f32, bool)>,
    /// 段落の**前後の空き**(pt)。docx の `w:spacing` の `w:before` / `w:after`
    /// (twips = pt × 20)。0 は「無指定」。
    ///
    /// **前は読んでも書いてもいなかった**(`w:line` だけ見ていた)ので、
    /// Word の文書を開いて保存すると段落の空きが黙って消えていた
    /// (2026-08-15 に見出しの詰まり方を直すとき見つけた)。
    pub space_before_pt: f32,
    pub space_after_pt: f32,
    /// 段落の背景色 `RRGGBB`(docx の w:shd)。見出しの背景色に使われる
    pub shade: Option<String>,
    /// 段落を枠で囲む(docx の w:pBdr)。囲みの注意書きに使われる。
    /// **どの辺かは [`Paragraph::border`] が持ちます** — こちらは
    /// 「囲みが付いているか」の1つの札で、画面の切り替えが使います
    pub boxed: bool,
    /// **段落の罫線の、どの辺を引くか**(docx の `w:pBdr` の子)。
    ///
    /// 記入欄の下線はこれです。内閣府の調査票の「記入日」以下の6行は
    /// `w:bottom` と `w:between` を持っていて、紙では欄の下に線が出ます。
    /// 前は4辺の区別が無く、紙にも出していませんでした(2026-09-01 発注者)。
    pub border: ParaBorder,
    /// ドロップキャップ(頭の1字を大きく)。docx では w:framePr の
    /// 「枠の段落+本文の段落」に割れるが、モデルでは1つの段落で持つ
    pub dropcap: bool,
    /// **AsciiDoc の原文のまま持ち越す行。**
    ///
    /// 本家にあってうちが扱わない書き方(註記・コードの塊・取り込みなど)は、
    /// 意味は分からなくても**字はそのまま返します**。ここに原文を持ち、
    /// [`crate::adoc::write`] がそのまま書き戻します。これが無いと、
    /// 開いて保存しただけで人の書いた AsciiDoc が壊れます
    /// (2026-08-18。`----` の塊に空行が入っていました)。
    /// docx の [`Paragraph::anchors`] と同じ「理解はしないが捨てない」
    pub raw_adoc: Option<String>,
}

impl Paragraph {
    /// 行間の倍率。0 や負が入っていても壊れない値を返す。
    pub fn spacing(&self) -> f32 {
        if self.line_spacing <= 0.0 { 1.0 } else { self.line_spacing.clamp(0.5, 5.0) }
    }

    /// 箇条書きの頭に付く印。組版のときに本文の前へ置く。
    /// **レベル(インデント)で印が変わる**(Word の複数レベルのリストの慣習)。
    pub fn marker(&self, nth: usize) -> Option<String> {
        // **文書が印を決めているなら、それを使います**(2026-08-31)。
        // `%1`〜`%9` はその段の番号です。docx の `w:lvlText` の書き方で、
        // どの段の番号かまでは見ずに、この段の番号を入れます
        if let Some(t) = self.list_text.as_deref().filter(|t| !t.is_empty()) {
            if self.list == ListKind::None {
                return None;
            }
            let mut out = String::with_capacity(t.len() + 2);
            let mut ji = t.chars().peekable();
            while let Some(c) = ji.next() {
                match (c, ji.peek()) {
                    ('%', Some(d)) if d.is_ascii_digit() => {
                        ji.next();
                        out.push_str(&(nth + 1).to_string());
                    }
                    _ => out.push(c),
                }
            }
            // 数字で終わる印は、本文とくっつかないように空白を1つ足します
            if out.ends_with(|c: char| c.is_ascii_digit()) {
                out.push(' ');
            }
            return Some(out);
        }
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
            // **日本の事務の様式の並び**(2026-08-25)。
            // 「1 →(1)→ ア →(ア)」が役所の文書の決まりで、Word の
            // 日本語の既定もこの順です。前は3段目が `1)` でした
            ListKind::Number => Some(match self.indent {
                0 => format!("{}. ", nth + 1),
                1 => format!("({}) ", nth + 1),
                2 => format!("{} ", katakana(nth)),
                3 => format!("({}) ", katakana(nth)),
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
#[derive(Debug, Clone)]
pub struct Cellbox {
    pub paragraphs: Vec<Paragraph>,
    /// このセルだけの罫線の指定(docx の `w:tcPr/w:tcBorders`)。
    /// `None` は「言わない」で、表の指定に従います
    pub borders: CellBorders,
    /// 横の結合(docx の w:gridSpan)。このセルが占める格子の列数。
    /// 0 と 1 はどちらも「結合なし」(既定の 0 を特別扱いしない)
    pub col_span: u8,
    /// 縦の結合
    pub v_merge: VMerge,
    /// **セルの中の縦の揃え**(docx の `w:tcPr/w:vAlign`)。
    ///
    /// 表計算と同じ型を使います([`book::VAlign`])。docx の既定は上揃えで、
    /// 表計算の既定は下揃えなので、**docx の既定は `Top`** です
    /// (読み書きの所で `None` に畳みます)。
    pub valign: book::VAlign,
}

/// **表のどこに罫線を引くか**(docx の `w:tblBorders`)。
///
/// 既定は6か所とも引きます。AsciiDoc の表と、新しく作る表がこれです。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableBorders {
    pub top: bool,
    pub left: bool,
    pub bottom: bool,
    pub right: bool,
    /// 行と行の間
    pub inside_h: bool,
    /// 列と列の間
    pub inside_v: bool,
}

impl Default for TableBorders {
    fn default() -> Self {
        TableBorders { top: true, left: true, bottom: true, right: true,
                       inside_h: true, inside_v: true }
    }
}

impl TableBorders {
    /// 1本も引かない
    pub fn nashi() -> Self {
        TableBorders { top: false, left: false, bottom: false, right: false,
                       inside_h: false, inside_v: false }
    }
}

/// **そのセルだけの罫線**(docx の `w:tcBorders`)。
///
/// `None` は「言わない」で、表の指定に従います。`Some(false)` は
/// **わざわざ引かない**で、意味が違います。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CellBorders {
    pub top: Option<bool>,
    pub left: Option<bool>,
    pub bottom: Option<bool>,
    pub right: Option<bool>,
}

/// **既定の縦位置は上揃え。**
///
/// `book::VAlign` の既定は下揃えですが、それは表計算の決めです。docx の
/// 既定は上揃えなので、`derive` に任せると新しいセルが全部
/// `w:vAlign val="bottom"` になります(2026-08-27 に実物の docx を
/// python-docx で開いて気づきました)。
impl Default for Cellbox {
    fn default() -> Self {
        Cellbox {
            paragraphs: Vec::new(),
            borders: CellBorders::default(),
            col_span: 0,
            v_merge: VMerge::None,
            valign: book::VAlign::Top,
        }
    }
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
    /// **どこに罫線を引くか**(docx の `w:tblBorders`)。
    ///
    /// 既定は6か所とも引きます。docx に `w:tblBorders` が書いてあれば、
    /// **そこに挙がっている辺だけ**引きます(2026-08-30)。前は指定を
    /// 読まずに必ず四方へ引いていたので、下線だけの様式が枠だらけに
    /// なっていました。
    pub borders: TableBorders,
    /// **`w:tblBorders` を自分では言っていない**という印。
    ///
    /// 言っていなければ、名乗っているスタイルの罫線に従います
    /// (`ooxml` が読んだ後で当てます)。AsciiDoc の表と新しく作る表は
    /// 自分で言っているので false です
    pub style_borders_unset: bool,
    /// 列の幅(mm)。docx の `w:gridCol`。空なら等分
    pub col_mm: Vec<f32>,
    /// **行の高さ(mm)**。docx の `w:trPr/w:trHeight`。空なら中身なり。
    ///
    /// 行より短ければ足りない分は既定、長ければ余りは捨てます
    /// (行を足し引きしても添字がずれないように、`rows` とは別に持ちます)。
    pub row_mm: Vec<f32>,
    /// **列の幅の割合**(AsciiDoc の `[cols="1,3"]`)。2026-08-18。
    ///
    /// adoc は幅を mm で言わず、比で言います。紙の幅が決まって初めて mm に
    /// なるので、比のまま持ち、テンプレートを合成するとき
    /// ([`crate::theme::compose_page`])に `col_mm` へ直します。
    /// docx から読んだ表は mm を持っているので、こちらは空です
    pub col_ratio: Vec<f32>,
    /// 表のスタイルの**名前だけ**(docx の `w:tblStyle w:val`)。
    /// 定義(styles.xml)は持たない主義のまま — 名前を運んで返すだけ。
    /// 読めた名前を書きで落とすと様式が崩れるので、往復のために持つ
    pub style: Option<String>,
    /// 表の置き方(docx の tblPr の `w:jc`)。None は指定なし(左)。
    /// 使うのは Left / Center / Right だけ(表の置き方に両端揃えは無い)
    pub align: Option<Align>,
    /// **表の役割**(AsciiDoc の `[.name]`)。2026-08-26。
    ///
    /// `.sheet.adoc` は「表1つ = シート1枚」なので、シートでない表
    /// (名前の定義・入力規則など)は印で見分けます。**名前は英語の
    /// 識別子**です — 表の題や列の見出しと違って、画面に出る字ではなく
    /// 書式の一部だからです。
    pub role: Option<String>,
    /// **表の題**(AsciiDoc の `.題`)。2026-08-18。
    ///
    /// calc のシート名になり、式の中では*表の名前*になる
    /// (`=SUM(売上台帳[金額])`)。HTML では `caption`。
    /// docx は表に題を持たないので、書き出しでは落ちる
    pub title: Option<String>,
    /// **1行目が見出しの行か。** AsciiDoc は「1行目の後ろに空行」で表します
    /// (2026-08-18)。HTML では `thead` になります。docx から読んだ表は
    /// いまのところ false です
    pub header_row: bool,
    /// 列幅を固定する(docx の `w:tblLayout w:type="fixed"`)。
    /// **裏返しで持つ** — docx の既定は autofit(要素なし)なので、
    /// `Default` の false がそのまま「autofit」になる
    pub fixed_layout: bool,
}

impl Table {
    /// 表の中身を**字の並び**にする(行優先)。
    ///
    /// 式の計算に載せるための形です。計算する側(`ops::table`)は
    /// `kumihan` を知らないままで済み、`kumihan` は `sheet` を知らないまま
    /// で済みます(SEKKEI「エンジンの統一」2段目の決め)。
    ///
    /// **結合したセルは、左上に字を置いて残りを空にします。**
    /// そうしないと結合の右にある列が1つずつずれ、見出しで引く
    /// 構造化参照(`売上台帳[金額]`)が別の列を指してしまいます。
    /// 表計算の結合セルも同じ持ち方です。
    pub fn text_rows(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|row| {
                let mut out = Vec::new();
                for c in row {
                    out.push(paras_text(&c.paragraphs));
                    // 結合で余分に占める格子の分だけ空を足す
                    for _ in 1..c.span() {
                        out.push(String::new());
                    }
                }
                out
            })
            .collect()
    }
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
    /// 文書の既定の字の大きさ(docx の `w:docDefaults` の `w:sz`)。
    /// run が `None` のときに効く。これも `None` なら [`DEFAULT_PT`]
    pub size_pt: Option<f32>,
    /// **文書の既定の段落後の空き**(docx の `w:docDefaults/w:pPrDefault` の
    /// `w:spacing w:after`。pt)。段落が自分で言っていなければこれを使います。
    ///
    /// python-docx の型紙は 10pt を書きます。読まないと段落が全部くっつきます
    /// (2026-09-03)
    pub space_after_pt: Option<f32>,
    /// **文書の既定の行間の倍率**(同じく `w:spacing w:line` ÷ 240)。
    /// python-docx の型紙は 276/240 = 1.15 です
    pub line_spacing: Option<f32>,
    /// **テーマの配色**(docx の `theme1.xml` の `a:clrScheme`)。
    /// 並びは `dk1 lt1 dk2 lt2 accent1..6 hlink folHlink` の12色です。
    ///
    /// 図形の色はこの名前(`<a:schemeClr val="accent1"/>`)で書いてあること
    /// が多く、読まないと Office の既定の色で出ます(2026-09-03)
    pub theme_colors: Vec<String>,
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
    /// **ページに貼り付く図形**(四角・楕円・矢印・チャートなど)。
    /// 本文を編集しても動きません — Word のページ固定の図形と同じです。
    ///
    /// 2026-08-29 発注者「docx の図形をやって」。それまで文書は
    /// [`Stroke`](手描きの筆)しか持てず、xlsx の側にだけ図形がありました
    pub shapes: Vec<DocShape>,
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
    /// **テンプレートの名前**(ネイティブ文書の頭の `:template: 名前`)。
    /// 実体は文書の隣 → ~/.config/officework/templates/ → 同梱の既定、の順に
    /// 探す(探すのはアプリの側 — 模型は名前を運ぶだけ)。docx には出ない
    pub template: Option<String>,
    /// **文書の頭の属性**(AsciiDoc の `:名前: 値`)。読んだ順に持ちます。
    ///
    /// AsciiDoc の文書は頭に属性を並べます(`:author:` `:revdate:` など)。
    /// **知らない名前も捨てません** — 捨てると、普通の AsciiDoc を開いて
    /// 保存しただけで持ち主の書いたことが消えます(anchors や sect_raw と
    /// 同じ「理解はしないが捨てない」)。`template` はここにも入り、
    /// [`Document::template`] からも引けます
    pub attrs: Vec<(String, String)>,
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

/// スタイルの名乗り(styles.xml の w:style の id・名前・種類)と、その見た目。
/// kind は docx の type のまま: "paragraph" / "character" / "table" / "numbering"
///
/// **見た目を持てるのは「自分で作ったスタイル」だけ**です(2026-08-27)。
/// 原本から読んだスタイルの定義は持ち越すだけで触りません(据え置きの決め)。
/// 使う人が `add_style` で作った物に色や大きさを持たせられないと、
/// 自作のスタイルが名前だけの物になります。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// 字の見た目。**設定した物だけ**を持ちます(None は「言わない」)
    pub look: StyleLook,
    /// 元になるスタイルの `styleId`(docx の `w:basedOn`)。
    /// 書いていない物は自分で全部決めます
    pub based_on: Option<String>,
    /// スタイルの一覧に出さない(docx の `w:semiHidden`)。
    /// **使ったら出す**(`w:unhideWhenUsed`)は別に持ちます
    pub hidden: bool,
    /// 使ったら一覧に出す(docx の `w:unhideWhenUsed`)
    pub unhide_when_used: bool,
    /// 書き替えを禁じる(docx の `w:locked`)。文書を保護したときに効きます
    pub locked: bool,
    /// リボンのスタイルの一覧に出す(docx の `w:qFormat`)
    pub quick_style: bool,
    /// **その種類の既定のスタイルか**(docx の `w:style w:default="1"`)。
    ///
    /// `w:pStyle` を名乗らない段落はこれに従います。読まないと
    /// `docDefaults` に落ち、内閣府の面談の記録では表の書体が
    /// Meiryo UI ではなく ＭＳ 明朝になっていました(2026-09-01)。
    pub default: bool,
    /// 一覧に並べる順(docx の `w:uiPriority`)。小さいほど先
    pub priority: Option<i32>,
    /// 段落の見た目(docx の `w:pPr`)。字の見た目([`StyleLook`])と対です
    pub para: StyleParaLook,
}

/// **Word が「使ったときに作る」組み込みスタイル。**
///
/// Word の空の文書には 33 個しかスタイルが入っていませんが、リボンの
/// スタイル一覧にはもっと並んでいます。並んでいるだけの物(潜在スタイル)は、
/// **貼った瞬間に styles.xml へ書き足されます**。だから Word では
/// 「List Bullet が無いので貼れない」ということが起きません。
///
/// ここはその一覧です。名前を渡すと (styleId, w:name) を返します。
/// 知らない名前には `None` を返すので、打ち間違いは今までどおり断ります
/// (2026-08-27、python-docx と同じ台本を通すために足しました)。
pub fn latent_style(name: &str) -> Option<(&'static str, &'static str)> {
    // (styleId, w:name)。**両方 Word の綴りのまま**でないと、読み手が
    // 組み込みの物だと見てくれません
    const LATENT: &[(&str, &str)] = &[
        ("ListBullet", "List Bullet"),
        // **`…Char` は文字スタイル。** 見出しや副題に文字だけ当てる
        // ときに Word が作ります(2026-08-28、連載のサンプルで踏みました)
        ("SubtitleChar", "Subtitle Char"),
        ("TitleChar", "Title Char"),
        ("QuoteChar", "Quote Char"),
        ("IntenseQuoteChar", "Intense Quote Char"),
        ("Heading1Char", "Heading 1 Char"),
        ("Heading2Char", "Heading 2 Char"),
        ("Heading3Char", "Heading 3 Char"),
        ("Heading4Char", "Heading 4 Char"),
        ("Heading5Char", "Heading 5 Char"),
        ("Heading6Char", "Heading 6 Char"),
        ("Strong", "Strong"),
        ("Emphasis", "Emphasis"),
        ("IntenseEmphasis", "Intense Emphasis"),
        ("SubtleEmphasis", "Subtle Emphasis"),
        ("BookTitle", "Book Title"),
        ("Hyperlink", "Hyperlink"),
        ("ListBullet2", "List Bullet 2"),
        ("ListBullet3", "List Bullet 3"),
        ("ListNumber", "List Number"),
        ("ListNumber2", "List Number 2"),
        ("ListNumber3", "List Number 3"),
        ("ListParagraph", "List Paragraph"),
        ("Caption", "Caption"),
        ("Subtitle", "Subtitle"),
        ("IntenseQuote", "Intense Quote"),
        ("NoSpacing", "No Spacing"),
        ("Header", "header"),
        ("Footer", "footer"),
        ("FootnoteText", "footnote text"),
        ("TOCHeading", "TOC Heading"),
        ("TOC1", "toc 1"),
        ("TOC2", "toc 2"),
        ("TOC3", "toc 3"),
    ];
    LATENT
        .iter()
        .find(|(id, n)| name.eq_ignore_ascii_case(id) || name.eq_ignore_ascii_case(n))
        .copied()
}

/// スタイルが持つ**段落の**見た目(docx の `w:pPr`)。
///
/// 三択(入・切・言わない)です。「言わない」は、元になるスタイル
/// (`basedOn`)から受け継ぐという意味で、`false` を書くと**わざわざ切る**
/// ことになり、意味が違います。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleParaLook {
    /// 横の揃え。`None` は「言わない」(元になるスタイルから受け継ぐ)
    pub align: Option<Align>,
    /// 段落の前後の空き(pt)
    pub space_before_pt: Option<f32>,
    pub space_after_pt: Option<f32>,
    /// 行間の倍率
    pub line_spacing: Option<f32>,
    /// 左のインデント段数(1段 = 全角2文字ぶん)
    pub indent: Option<u8>,
    /// 1行目の字下げ(twip。負はぶら下げ)
    pub first_line_twips: Option<i32>,
    /// **箇条書きの種類**(docx の `w:pPr/w:numPr/w:numId` を
    /// `numbering.xml` で引いた結果)。
    ///
    /// python-docx の `add_paragraph(style="List Bullet")` は本文に
    /// `w:numPr` を書きません。中黒も番号もスタイルの側にあります。
    /// 読まないと、箇条書きが**ただの段落**になります(2026-09-03)
    pub list: Option<ListKind>,
    /// その印の字(`w:lvlText`。`●` や `1.`)。無ければ種類なりの既定
    pub list_text: Option<String>,
    /// **同じスタイルの段落が続く間は、前後の空きを入れない**
    /// (docx の `w:pPr/w:contextualSpacing`)。
    ///
    /// 箇条書きの項目どうしが離れないための指定です。読まないと、
    /// 文書の既定の「段落後 10pt」が項目ごとに入って間延びします
    /// (2026-09-03)
    pub contextual_spacing: Option<bool>,
    /// **段落の罫線**(docx の `w:pPr/w:pBdr`)。
    ///
    /// python-docx の既定の型紙では、題(`Title`)の下の線がここにあります。
    /// 本文には1文字も書いてありません
    pub border: Option<ParaBorder>,
}

/// スタイルが持つ字の見た目(docx の `w:rPr`)。三択(入・切・言わない)です。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleLook {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// 字の大きさ(pt)
    pub size_pt: Option<f32>,
    /// 字の色(RRGGBB)
    pub color: Option<String>,
    /// 書体の名前
    pub font: Option<String>,
    /// 背景の塗り(RRGGBB)
    pub fill: Option<String>,
}

impl StyleLook {
    /// 何も言っていないか(何も無ければ `w:rPr` を書きません)
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
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

/// **ページに貼り付く図形1つ。** 座標はそのページの中の mm(紙の左上が原点)。
///
/// 形・塗り・線・中の文字は [`book::SheetShape`] をそのまま使います。
/// 表と文書で図形の形が食い違わないためです — 組む所も
/// [`paper::grid`] の同じ1本を通ります。
///
/// `look` の `at` と `dx_px` / `dy_px` は**使いません**。文書の図形は
/// セルに留めるのではなく、ページの上の mm で置くからです。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocShape {
    /// 何ページ目か(0始まり)
    pub page: usize,
    /// ページの左上からの mm
    pub x_mm: f32,
    pub y_mm: f32,
    /// 大きさ(mm)
    pub w_mm: f32,
    pub h_mm: f32,
    /// 形・塗り・線・中の文字
    pub look: book::SheetShape,
}

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

impl Stroke {
    /// 筆の通り道の外接する四角(mm)。`(x0, y0, x1, y1)`
    pub fn bbox(&self) -> Option<(f32, f32, f32, f32)> {
        let mut it = self.points.iter();
        let (x, y) = it.next()?;
        let (mut x0, mut y0, mut x1, mut y1) = (*x, *y, *x, *y);
        for (x, y) in it {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        Some((x0, y0, x1, y1))
    }
}

/// **筆の線を SVG にする**(2026-08-18)。
///
/// ネイティブ文書(.adoc)は手描きの線を持てないので、保存のときに絵に
/// します。`image::` で置けば、HTML にも PDF にも docx にも画像として乗り、
/// 後から消せます。独自の書き方を1つも足さずに済むのが選んだ理由です。
///
/// 座標は mm。返るのは(SVG の字, 幅 mm, 高さ mm)で、線が1本も無ければ
/// `None`。太さと色は画面の筆と同じです(ペン 0.45mm の濃紺、蛍光ペン
/// 3mm の薄い黄)。
pub fn strokes_to_svg(strokes: &[&Stroke]) -> Option<(String, f32, f32)> {
    // 外接する四角。線の太さのぶんだけ外へ広げる(端が切れないように)
    let mut bb: Option<(f32, f32, f32, f32)> = None;
    for st in strokes {
        let Some((ax, ay, bx, by)) = st.bbox() else { continue };
        let half = if st.highlighter { 1.5 } else { 0.25 };
        let (ax, ay, bx, by) = (ax - half, ay - half, bx + half, by + half);
        bb = Some(match bb {
            None => (ax, ay, bx, by),
            Some((x0, y0, x1, y1)) => (x0.min(ax), y0.min(ay), x1.max(bx), y1.max(by)),
        });
    }
    let (x0, y0, x1, y1) = bb?;
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}mm\" height=\"{h:.2}mm\" \
viewBox=\"0 0 {w:.2} {h:.2}\">"
    );
    for st in strokes {
        if st.points.is_empty() {
            continue;
        }
        let mut d = String::new();
        for (i, (x, y)) in st.points.iter().enumerate() {
            let mark = if i == 0 { 'M' } else { 'L' };
            d.push_str(&format!("{mark}{:.2} {:.2} ", x - x0, y - y0));
        }
        let (colour, weight, alpha) = if st.highlighter {
            ("#FFE65A", 3.0, 0.35)
        } else {
            ("#1C3B52", 0.45, 1.0)
        };
        s.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"{weight}\" \
stroke-opacity=\"{alpha}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
            d.trim_end()
        ));
    }
    s.push_str("</svg>");
    Some((s, w, h))
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
    /// **この部品に錨を下ろした図形の原文。** 段落と同じ「理解はしないが
    /// 捨てない」で、`w:drawing` を丸ごと控えます。
    ///
    /// 紙の飾り枠はここに入ります。内閣府の面談の記録は、紙の全面の
    /// 長方形をヘッダーに置いていて、前は捨てていました
    /// (2026-09-01 発注者「document_4 でも囲みが表示できていない」)。
    /// **すべての紙に出します** — ヘッダーとはそういうものです
    pub anchors: Vec<String>,
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
pub fn set_paras_text(paras: &mut Vec<Paragraph>, text: &str) {
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
                .unwrap_or((None, None, CharFormat::default()));
            p.runs = vec![Run { text: s.to_string(), size_pt: pt, font, fmt }];
            p
        })
        .collect();
}

impl Document {
    /// **文書に出てくる字を全部、順に見る**(表の中も含む)。
    ///
    /// 書体を選ぶときに使います。どの文字の種類が要るかは、文中の字を
    /// 見ないと分かりません([`crate::font::for_text`])。
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.blocks.iter().flat_map(|b| {
            let paras: Vec<&Paragraph> = match b {
                Block::Para(p) => vec![p],
                Block::Table(t) => t
                    .rows
                    .iter()
                    .flatten()
                    .flat_map(|c| c.paragraphs.iter())
                    .collect(),
            };
            paras
                .into_iter()
                .flat_map(|p| p.runs.iter().flat_map(|r| r.text.chars()))
                .collect::<Vec<_>>()
        })
    }
    /// 段落だけを順に見る(組版は v0 では段落のみを組む)
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Para(p) => Some(p),
            Block::Table(_) => None,
        })
    }
    /// 段落を書ける形で順に見る(表は飛ばす — [`paragraphs`](Self::paragraphs)
    /// と同じ数え方なので、番号がそのまま通じる)。
    pub fn paragraphs_mut(&mut self) -> impl Iterator<Item = &mut Paragraph> {
        self.blocks.iter_mut().filter_map(|b| match b {
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
    /// 名前(`w:tag`)の付いた記入欄に字を入れる。返りは書いた欄の数。
    ///
    /// 同じ名前の欄が何か所あっても全部に書きます(表紙と2枚目に同じ欄が
    /// ある様式のため)。欄が複数の run に割れていれば、最初の run に
    /// 入れて残りは空にします。段落の中も表の中も見ます。
    pub fn set_sdt_text(&mut self, tag: &str, value: &str) -> usize {
        fn put(p: &mut Paragraph, tag: &str, value: &str) -> usize {
            let mut n = 0;
            let mut first = true;
            for r in &mut p.runs {
                let hit = r.fmt.sdt.as_deref().is_some_and(|s| s.tag == tag);
                if !hit {
                    first = true;
                    continue;
                }
                if first {
                    r.text = value.to_string();
                    n += 1;
                    first = false;
                } else {
                    r.text.clear();
                }
            }
            n
        }
        let mut n = 0;
        for b in &mut self.blocks {
            match b {
                Block::Para(p) => n += put(p, tag, value),
                Block::Table(t) => {
                    for row in &mut t.rows {
                        for c in row {
                            for p in &mut c.paragraphs {
                                n += put(p, tag, value);
                            }
                        }
                    }
                }
            }
        }
        n
    }

    pub fn set_body_text(&mut self, text: &str) {
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
        self.splice_text(pre, ob.len() - suf, &text[pre..nb.len() - suf]);
    }

    /// 本文の `start..end`(バイト。段落は \n で繋いだ物差し)を `insert` で
    /// 置き換える。run の境と段落の性質を保つ、編集モデルの心臓。
    pub fn splice_text(&mut self, start: usize, end: usize, insert: &str) {
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
                runs: vec![Run { text: insert.to_string(), size_pt: None, font: None,
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
            .unwrap_or((None, None, CharFormat::default()));
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
            normalize_runs(&mut p.runs);
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
        let pt = p.runs.first().and_then(|r| r.size_pt);

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
        normalize_runs(&mut p.runs);
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
            p.runs = left;
            p.runs.extend(mid);
            p.runs.extend(right);
            normalize_runs(&mut p.runs);
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
        // 手で大きさを変えた瞬間から「指定あり」になる(Word と同じ)。
        // 無指定の run は文書の既定を出発点にする
        let base = self.base_pt();
        if range.start < range.end {
            self.apply_runs(range, |r| r.size_pt = Some(f(r.pt(base)).clamp(4.0, 400.0)));
            return;
        }
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if !target.contains(&i) {
                continue;
            }
            if let Block::Para(p) = b {
                for r in &mut p.runs {
                    r.size_pt = Some(f(r.pt(base)).clamp(4.0, 400.0));
                }
            }
        }
    }

    /// 選択範囲の文字の大きさを指定し直す(選択の字にだけ。空なら段落まるごと)。
    /// `None` は**指定を外す** — 文書の既定に従う字に戻る(標準スタイルの形)。
    pub fn set_size(&mut self, range: std::ops::Range<usize>, pt: Option<f32>) {
        let v = pt.map(|p| p.clamp(4.0, 400.0));
        if range.start < range.end {
            self.apply_runs(range, |r| r.size_pt = v);
            return;
        }
        let target = self.para_range(range);
        for (i, b) in self.blocks.iter_mut().filter(|b| matches!(b, Block::Para(_))).enumerate() {
            if !target.contains(&i) {
                continue;
            }
            if let Block::Para(p) = b {
                for r in &mut p.runs {
                    r.size_pt = v;
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

    /// いま選択範囲の文字の大きさ(表示用に解決済み — 無指定なら文書の既定)。
    pub fn size_at(&self, range: std::ops::Range<usize>) -> Option<f32> {
        self.run_at(range.start).map(|r| r.pt(self.base_pt()))
    }

    /// 選択範囲の字の大きさ。**下敷きの大きさを外から渡します。**
    ///
    /// [`size_at`](Self::size_at) は文書自身の大きさしか下敷きにできません。
    /// テンプレートが決めた大きさは文書の中に無いので、それを見せたい所
    /// (リボンの大きさの欄など)はこちらを使ってください。渡さないと、
    /// 言語ごとに変えた大きさが画面に出ません(2026-08-26)。
    pub fn size_at_with(&self, range: std::ops::Range<usize>, base: f32) -> f32 {
        self.run_at(range.start).map(|r| r.pt(base)).unwrap_or(base)
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
        // **選んでいるときは、選んだ字の書式を返します。**
        //
        // `run_at` は「カーソルの直前の字」を返します。カーソルが1点のときは
        // それが正しい(打つとその書式になる、という慣習)のですが、範囲を
        // 選んでいるときに使うと、**選んだ字の1つ手前**を見てしまいます。
        //
        // そのせいで、太字の語を選んで太字のボタンを押しても外れませんでした
        // (手前の字が太字でないので「いまは太字でない」と判断し、また
        // 太字を掛けていた)。2026-08-17 発注者「書式設定が戻せない」。
        if range.start != range.end {
            if let Some(r) = self.run_from(range.start) {
                return r.fmt.clone();
            }
        }
        self.run_at(range.start).map(|r| r.fmt.clone()).unwrap_or_default()
    }

    /// その位置から**始まる字**を持つ run(`run_at` と違い、手前は見ません)。
    fn run_from(&self, pos: usize) -> Option<&Run> {
        let mut at = 0usize;
        for p in self.paragraphs() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            if pos < at + len {
                let off = pos - at;
                let mut racc = 0usize;
                for r in &p.runs {
                    let rend = racc + r.text.len();
                    if off < rend {
                        return Some(r);
                    }
                    racc = rend;
                }
            }
            at += len + 1;
        }
        None
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
            normalize_runs(&mut p.runs);
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
pub(super) fn normalize_runs(runs: &mut Vec<Run>) {
    // 字を全部消しても、段落は自分の大きさを覚えている(指定があれば)
    let keep_pt = runs.first().and_then(|r| r.size_pt);
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
        out.push(Run { text: String::new(), size_pt: keep_pt, font: None,
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

    /// 平文から文書を作る。字の大きさは**指定しない**(文書の既定に従う)。
    /// ここで数を入れると、作っただけの文書が「大きさ指定つき」になる
    pub fn plain(text: &str) -> Document {
        Document {
            blocks: text
                .split('\n')
                .map(|p| Block::Para(Paragraph {
                    line_spacing: 1.0,
                    runs: vec![Run { text: p.to_string(), size_pt: None, font: None, fmt: Default::default() }],
                    ..Default::default() }))
                .collect(),
            ..Default::default()
        }
    }

    /// 画面・紙に写すときの、この文書の基準の大きさ(pt)。
    /// docDefaults にあればそれ、無ければ [`DEFAULT_PT`]
    pub fn base_pt(&self) -> f32 {
        self.size_pt.unwrap_or(DEFAULT_PT)
    }

    /// **段落スタイルが言う字の大きさ(pt)。**
    ///
    /// docx は3段で決めます。run の `w:rPr` が一番強く、次が段落スタイルの
    /// `w:rPr`、最後が `docDefaults` です。真ん中を読んでいなかったので、
    /// 内閣府の調査票が 12pt ではなく 11pt で組まれていました
    /// (2026-09-01。本文のスタイル Body Text が `w:sz 24` を言っています)。
    ///
    /// 元になるスタイル(`w:basedOn`)をたどります。輪になっている定義でも
    /// 止まるよう、たどる回数を限ります。
    pub fn style_pt(&self, id: Option<&str>) -> Option<f32> {
        self.style_look(id, |l| l.size_pt)
    }

    /// **段落スタイルが言う書体の名前。** 決め方は [`Document::style_pt`] と
    /// 同じです。この文書は Normal が「ＭＳ 明朝」と言っていて、run は
    /// 何も言っていません。行送りは原本の書体で決まるので、ここが引けないと
    /// 書体から高さを出せません。
    pub fn style_font(&self, id: Option<&str>) -> Option<String> {
        self.style_look(id, |l| l.font.clone())
    }

    /// **スタイル定義を1つにまとめる。** 元になるスタイル(`w:basedOn`)を
    /// たどり、手前のスタイルが言っていない所だけ先のスタイルで埋めます。
    ///
    /// この文書がその名前のスタイルを持っていなければ `None` です。
    /// テンプレートを重ねてよいかどうかの判断にも使います
    /// ([`crate::theme::compose`])。
    pub fn style_matome(&self, id: &str) -> Option<(StyleLook, StyleParaLook)> {
        let hiku =
            |i: &str| self.styles.iter().chain(self.styles_new.iter()).find(|s| s.id == i);
        let mut lk = StyleLook::default();
        let mut pl = StyleParaLook::default();
        let mut ima = id;
        let mut atta = false;
        for _ in 0..16 {
            let Some(s) = hiku(ima) else { break };
            atta = true;
            lk.bold = lk.bold.or(s.look.bold);
            lk.italic = lk.italic.or(s.look.italic);
            lk.underline = lk.underline.or(s.look.underline);
            lk.strike = lk.strike.or(s.look.strike);
            lk.size_pt = lk.size_pt.or(s.look.size_pt);
            lk.color = lk.color.clone().or_else(|| s.look.color.clone());
            lk.font = lk.font.clone().or_else(|| s.look.font.clone());
            lk.fill = lk.fill.clone().or_else(|| s.look.fill.clone());
            pl.align = pl.align.or(s.para.align);
            pl.space_before_pt = pl.space_before_pt.or(s.para.space_before_pt);
            pl.space_after_pt = pl.space_after_pt.or(s.para.space_after_pt);
            pl.line_spacing = pl.line_spacing.or(s.para.line_spacing);
            pl.indent = pl.indent.or(s.para.indent);
            pl.first_line_twips = pl.first_line_twips.or(s.para.first_line_twips);
            pl.list = pl.list.or(s.para.list);
            pl.list_text = pl.list_text.clone().or_else(|| s.para.list_text.clone());
            pl.contextual_spacing = pl.contextual_spacing.or(s.para.contextual_spacing);
            pl.border = pl.border.or(s.para.border);
            match s.based_on.as_deref() {
                Some(o) => ima = o,
                None => break,
            }
        }
        atta.then_some((lk, pl))
    }

    /// スタイルの見た目を、元になるスタイル(`w:basedOn`)をたどって引く。
    /// 輪になっている定義でも止まるよう、たどる回数を限ります。
    fn style_look<T>(&self, id: Option<&str>, toru: impl Fn(&StyleLook) -> Option<T>) -> Option<T> {
        let hiku =
            |i: &str| self.styles.iter().chain(self.styles_new.iter()).find(|s| s.id == i);
        // **名乗らない段落は、その種類の既定のスタイル**に従います
        let kitei = || {
            self.styles
                .iter()
                .chain(self.styles_new.iter())
                .find(|s| s.default && s.kind == "paragraph")
                .map(|s| s.id.as_str())
        };
        let mut ima = match id {
            Some(i) => i,
            None => kitei()?,
        };
        for _ in 0..16 {
            let s = hiku(ima)?;
            if let Some(v) = toru(&s.look) {
                return Some(v);
            }
            ima = s.based_on.as_deref()?;
        }
        None
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
    /// **塗る四角**(セルの背景・表の帯)。([x, y, 幅, 高さ] mm, RRGGBB)。
    ///
    /// **罫線より先に敷きます** — 後にすると線を塗り潰します。
    /// 文書の表はまだ使いませんが、表計算の紙面が同じ形に載るために
    /// 要ります(2026-08-27 発注者「文書の方の grid と同じでないですか」)。
    pub fills: Vec<([f32; 4], String)>,
    /// **見出しの行を持つ表の番号。** 表が頁をまたぐとき、次の頁の頭に
    /// 見出しの行を繰り返します(2026-08-27 発注者「タイトル行の繰り返しも
    /// 同じものがあるはず」)。`Table::header_row` は模型に在り、adoc も
    /// HTML も docx も見ていましたが、**紙だけが見ていません**でした
    pub header_tables: Vec<usize>,
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

/// 番号をカタカナにする(ア・イ・ウ…)。**事務の様式の3段目**です。
///
/// 五十音の 45 字を順に使い、それより多いときは「ア1」のように数を足します
/// (使い切って番号が消えるより、重ならないほうがましです)。
fn katakana(nth: usize) -> String {
    const KANA_A: [&str; 45] = [
        "ア", "イ", "ウ", "エ", "オ", "カ", "キ", "ク", "ケ", "コ",
        "サ", "シ", "ス", "セ", "ソ", "タ", "チ", "ツ", "テ", "ト",
        "ナ", "ニ", "ヌ", "ネ", "ノ", "ハ", "ヒ", "フ", "ヘ", "ホ",
        "マ", "ミ", "ム", "メ", "モ", "ヤ", "ユ", "ヨ",
        "ラ", "リ", "ル", "レ", "ロ", "ワ", "ヲ",
    ];
    match nth / KANA_A.len() {
        0 => KANA_A[nth].to_string(),
        round_of => format!("{}{}", KANA_A[nth % KANA_A.len()], round_of),
    }
}
