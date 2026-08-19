//! **フォルダの中身を並べる**(SEKKEI「画面を1つにする」2段目)。
//!
//! officework はフォルダを開いて使います。右パネルにこの一覧を出し、
//! 選んだファイルを開きます。
//!
//! *ファイルの種類は名前で決まります。* 二重の拡張子の決め(2026-08-18)を
//! ここで解きます。中身を見て当てることはしません — 名前で決まるほうが、
//! 使う人が次に何が起きるか分かります。
//!
//! この層は絵を描きません。並べる所までがここの仕事で、描くのはアプリです。

use std::path::{Path, PathBuf};

/// ファイルの種類。**どの画面で開くか**がこれで決まります。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// フォルダ
    Folder,
    /// 文書(`名前.adoc`)
    Doc,
    /// 表(`名前.sheet.adoc`)
    Sheet,
    /// 見た目の元(`名前.tmpl.adoc`)
    Tmpl,
    /// 穴つきの枠(`名前.form.adoc`)
    Form,
    /// 受け渡しの文書(`.docx`)
    DocX,
    /// 受け渡しの表(`.xlsx`)
    SheetX,
    /// 大きい表のデータ(`.parquet`)
    Data,
    /// 画像
    Image,
    /// プログラム(`.py`)
    Script,
    /// それ以外
    Other,
}

impl Kind {
    /// 表の画面で開く種類か。
    pub fn is_sheet(self) -> bool {
        matches!(self, Kind::Sheet | Kind::SheetX)
    }

    /// 文書の画面で開く種類か。見た目の元と様式も文書です。
    pub fn is_doc(self) -> bool {
        matches!(self, Kind::Doc | Kind::Tmpl | Kind::Form | Kind::DocX)
    }

    /// この種類を開けるか(開けない物は一覧で薄く出します)。
    pub fn can_open(self) -> bool {
        self.is_sheet() || self.is_doc()
    }

    /// 一覧に出す短い札。
    pub fn label(self) -> &'static str {
        match self {
            Kind::Folder => lang::i18n::tr("フォルダ"),
            Kind::Doc => lang::i18n::tr("文書"),
            Kind::Sheet => lang::i18n::tr("表"),
            Kind::Tmpl => lang::i18n::tr("見た目"),
            Kind::Form => lang::i18n::tr("様式"),
            Kind::DocX => "docx",
            Kind::SheetX => "xlsx",
            Kind::Data => "parquet",
            Kind::Image => lang::i18n::tr("画像"),
            Kind::Script => "Python",
            Kind::Other => "",
        }
    }
}

/// 一覧の1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 画面に出す名前。**二重の拡張子は落とします**(`売上台帳.sheet.adoc`
    /// は「売上台帳」)。種類は札で分かるので、名前に二度出しません
    pub name: String,
    /// 元のファイル名(開くときに使う)
    pub file_name: String,
    pub path: PathBuf,
    pub kind: Kind,
}

/// ファイル名から種類を決める。
///
/// **中身は見ません。** 名前だけで決めるので、開く前に分かります。
pub fn kind_of(file_name: &str) -> Kind {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".sheet.adoc") {
        return Kind::Sheet;
    }
    if lower.ends_with(".tmpl.adoc") {
        return Kind::Tmpl;
    }
    if lower.ends_with(".form.adoc") {
        return Kind::Form;
    }
    if lower.ends_with(".adoc") {
        return Kind::Doc;
    }
    match lower.rsplit_once('.').map(|(_, e)| e) {
        Some("docx") => Kind::DocX,
        Some("xlsx" | "xltx") => Kind::SheetX,
        Some("parquet") => Kind::Data,
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp") => Kind::Image,
        Some("py") => Kind::Script,
        _ => Kind::Other,
    }
}

/// 画面に出す名前。二重の拡張子を落とします。
pub fn display_name(file_name: &str, kind: Kind) -> String {
    let cut = |suffix: &str| file_name[..file_name.len() - suffix.len()].to_string();
    let lower = file_name.to_ascii_lowercase();
    match kind {
        Kind::Sheet if lower.ends_with(".sheet.adoc") => cut(".sheet.adoc"),
        Kind::Tmpl if lower.ends_with(".tmpl.adoc") => cut(".tmpl.adoc"),
        Kind::Form if lower.ends_with(".form.adoc") => cut(".form.adoc"),
        Kind::Doc if lower.ends_with(".adoc") => cut(".adoc"),
        _ => file_name.to_string(),
    }
}

/// **保存するときのブックの名前。** [`kind_of`] の逆向きです。
///
/// 保存の窓は使う人が名前を打つので、`売上台帳` とも `売上台帳.adoc` とも
/// 打たれます。どちらもそのまま書くと、`kind_of` は「文書」と読みます —
/// **保存した表が一覧で文書に化ける**(2026-08-19 の引き継ぎに実害として
/// 挙がっていた件)。ここを通して `名前.sheet.adoc` に揃えます。
///
/// 種類を決めるのが `kind_of` の1箇所なら、種類に合う名前を作るのも
/// 1箇所であるべきです。**片方だけ直すと、また食い違います。**
///
/// `.xlsx` のような別の形式は**触りません** — 呼ぶ側が「AsciiDoc で書く」と
/// 決めたときにだけ通してください。
pub fn as_sheet_adoc(path: &Path) -> PathBuf {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return path.to_path_buf();
    };
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".sheet.adoc") {
        return path.to_path_buf();
    }
    // `.adoc` で終わるなら、その `.adoc` を二重の拡張子に差し替える。
    // `.tmpl.adoc` `.form.adoc` もここに入る — **表は見た目の元でも様式でも
    // ない**ので、ブックとして保存する以上は表の名前にします
    let stem = for_suffix(&lower, name, ".tmpl.adoc")
        .or_else(|| for_suffix(&lower, name, ".form.adoc"))
        .or_else(|| for_suffix(&lower, name, ".adoc"))
        .unwrap_or(name);
    path.with_file_name(format!("{stem}.sheet.adoc"))
}

/// 末尾が合えば、それを落とした頭を返す(合わなければ `None`)
fn for_suffix<'a>(lower: &str, name: &'a str, suffix: &str) -> Option<&'a str> {
    lower.ends_with(suffix).then(|| &name[..name.len() - suffix.len()])
}

/// フォルダの中身を並べる。
///
/// *並びは「フォルダが先、次に名前順」*です。隠しファイル(`.` で始まる)と
/// このアプリの控え(`.jo-history`)は出しません。
///
/// 読めないフォルダのときは空を返します。**画面を止めません** —
/// 一覧が空なのは、使う人には「何も無い」と同じに見えます。
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = Vec::new();
    for e in rd.flatten() {
        let file_name = e.file_name().to_string_lossy().to_string();
        if file_name.starts_with('.') {
            continue;
        }
        let path = e.path();
        let kind = if path.is_dir() { Kind::Folder } else { kind_of(&file_name) };
        out.push(Entry { name: display_name(&file_name, kind), file_name, path, kind });
    }
    out.sort_by(|a, b| {
        let 順 = |k: Kind| if k == Kind::Folder { 0 } else { 1 };
        順(a.kind).cmp(&順(b.kind)).then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// 開けるファイルだけを並べる(一覧を絞りたいとき)。
pub fn openable(dir: &Path) -> Vec<Entry> {
    list(dir).into_iter().filter(|e| e.kind.can_open() || e.kind == Kind::Folder).collect()
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 二重の拡張子で種類が決まる() {
        assert_eq!(kind_of("報告書.adoc"), Kind::Doc);
        assert_eq!(kind_of("売上台帳.sheet.adoc"), Kind::Sheet);
        assert_eq!(kind_of("既定.tmpl.adoc"), Kind::Tmpl);
        assert_eq!(kind_of("申込書.form.adoc"), Kind::Form);
        assert_eq!(kind_of("送付状.docx"), Kind::DocX);
        assert_eq!(kind_of("在庫.xlsx"), Kind::SheetX);
        assert_eq!(kind_of("売上.parquet"), Kind::Data);
        assert_eq!(kind_of("写真.png"), Kind::Image);
        assert_eq!(kind_of("集計.py"), Kind::Script);
        assert_eq!(kind_of("覚え書き.txt"), Kind::Other);
    }

    /// **大文字でも同じ**(Windows から来たファイル)
    #[test]
    fn 大文字小文字を見ない() {
        assert_eq!(kind_of("台帳.SHEET.ADOC"), Kind::Sheet);
        assert_eq!(kind_of("表.XLSX"), Kind::SheetX);
    }

    #[test]
    fn 名前から二重の拡張子を落とす() {
        assert_eq!(display_name("売上台帳.sheet.adoc", Kind::Sheet), "売上台帳");
        assert_eq!(display_name("報告書.adoc", Kind::Doc), "報告書");
        assert_eq!(display_name("既定.tmpl.adoc", Kind::Tmpl), "既定");
        // 受け渡しの形は拡張子を残す(元の形が分かるほうがよい)
        assert_eq!(display_name("送付状.docx", Kind::DocX), "送付状.docx");
    }

    /// 保存の名前は `kind_of` が表と読める形になる。**この2つは対**
    #[test]
    fn 保存の名前は表の形になる() {
        let n = |s: &str| {
            as_sheet_adoc(Path::new(s)).file_name().unwrap().to_str().unwrap().to_string()
        };
        // 拡張子を打たなかったとき
        assert_eq!(n("売上台帳"), "売上台帳.sheet.adoc");
        // `.adoc` とだけ打ったとき(**これが実害の出ていた道**)
        assert_eq!(n("売上台帳.adoc"), "売上台帳.sheet.adoc");
        // もう表の形なら触らない
        assert_eq!(n("売上台帳.sheet.adoc"), "売上台帳.sheet.adoc");
        // 表は見た目の元でも様式でもない
        assert_eq!(n("売上台帳.tmpl.adoc"), "売上台帳.sheet.adoc");
        assert_eq!(n("売上台帳.form.adoc"), "売上台帳.sheet.adoc");
        // 大文字でも同じ
        assert_eq!(n("売上台帳.ADOC"), "売上台帳.sheet.adoc");
        // 名前に点があっても頭を落とさない
        assert_eq!(n("2026.08 売上"), "2026.08 売上.sheet.adoc");
        // 置き場は動かさない
        assert_eq!(as_sheet_adoc(Path::new("/tmp/帳簿/売上.adoc")),
                   Path::new("/tmp/帳簿/売上.sheet.adoc"));
    }

    /// **付けた名前を `kind_of` が表と読む。** ここが対でないと化ける
    #[test]
    fn 保存の名前と種類の判定が対になっている() {
        for s in ["売上台帳", "売上台帳.adoc", "売上台帳.sheet.adoc", "2026.08 売上"] {
            let made = as_sheet_adoc(Path::new(s));
            let name = made.file_name().unwrap().to_str().unwrap();
            assert_eq!(kind_of(name), Kind::Sheet, "{s} → {name} が表と読めない");
            assert_eq!(display_name(name, Kind::Sheet), s.trim_end_matches(".adoc")
                .trim_end_matches(".sheet"), "{s} の見せ名");
        }
    }

    /// **`.sheet.adoc` を文書と間違えない。** どちらも `.adoc` で終わる
    #[test]
    fn 表を文書と間違えない() {
        assert!(kind_of("売上台帳.sheet.adoc").is_sheet());
        assert!(!kind_of("売上台帳.sheet.adoc").is_doc());
        assert!(kind_of("報告書.adoc").is_doc());
        assert!(!kind_of("報告書.adoc").is_sheet());
    }

    /// 見た目の元と様式は**文書の画面**で開く
    #[test]
    fn 見た目と様式は文書() {
        assert!(kind_of("既定.tmpl.adoc").is_doc());
        assert!(kind_of("申込書.form.adoc").is_doc());
    }

    #[test]
    fn 開けない種類が分かる() {
        assert!(!kind_of("売上.parquet").can_open());
        assert!(!kind_of("写真.png").can_open());
        assert!(kind_of("報告書.adoc").can_open());
    }

    #[test]
    fn フォルダを並べる() {
        let dir = std::env::temp_dir().join(format!("jo-folder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        for f in ["報告書.adoc", "売上台帳.sheet.adoc", "既定.tmpl.adoc", ".隠し", "覚え.txt"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        std::fs::create_dir_all(dir.join(".jo-history")).unwrap();

        let v = list(&dir);
        let 名: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(名[0], "images", "フォルダが先に来ていない: {名:?}");
        assert!(!名.contains(&".隠し"), "隠しファイルが出た: {名:?}");
        assert!(!名.iter().any(|n| n.contains("jo-history")), "控えが出た: {名:?}");
        assert!(名.contains(&"売上台帳"), "{名:?}");
        assert!(名.contains(&"覚え.txt"), "開けない物も一覧には出す: {名:?}");

        // 開ける物だけに絞れる
        let v2 = openable(&dir);
        assert!(!v2.iter().any(|e| e.name == "覚え.txt"), "絞れていない");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 読めないフォルダでも落ちない
    #[test]
    fn 無いフォルダでも落ちない() {
        assert!(list(Path::new("/そんなフォルダは無い")).is_empty());
    }
}
