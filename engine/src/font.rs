//! フォント — **文書が名前で指定し、機械にある実体を探して結び付ける。**
//!
//! office のフォントはアプリの好みではなく**文書の設定**である。
//! docx なら `w:rFonts`、xlsx なら `<font><name>` に名前が入っている。
//! だからここがやるのは2つだけ:
//!
//!   1. この機械にどんなフォントがあるか数える([`list`]) — リボンの一覧になる
//!   2. 名前から実体を引く([`resolve`]) — 文書が指定した書体を出す
//!
//! **同梱はしない。** 実行ファイルに埋め込むと、それはフォントを配ることになる。
//!
//! 文書が指定した書体がこの機械に無いときは [`fallback`] に落ちるが、
//! **落ちたことは黙らない**(別の書体で出したなら、そう言う必要がある)。
//!
//! 「アプリの好みで優先順を決める」形にしていた時期があるが、それは誤り。
//! office のフォントは利用者が選び、文書が覚えているもの。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 使えるフォント1つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    /// 書体の名前。文書に書かれるのはこれ(日本語名があればそちら)
    pub name: String,
    /// 英語(ASCII)の書体名。**画面の絞り込みで日本語名と両方引くための副名。**
    /// [`name`] が既に ASCII のときは同じ字。ASCII 名が無い書体では [`name`] と同じ
    pub ascii: String,
    pub path: PathBuf,
    /// ttc(まとめ)の中の何番目か
    pub index: u32,
    /// 日本語の字を持っているか
    pub japanese: bool,
    /// 漢字を持っているか(中国語もこれで見ます)
    pub han: bool,
    /// ハングルを持っているか
    pub hangul: bool,
    /// キリル文字を持っているか
    pub cyrillic: bool,
    /// ラテン文字を、記号つきまで持っているか(ä é などが要る言語のため)
    pub latin: bool,
    /// ベトナム語の字を持っているか。ラテン文字ですが、記号が二重に付く
    /// 字(ế など)は持っていない書体が多いので別に見ます
    pub vietnamese: bool,
    /// 太字・斜体でない、素の書体か
    pub regular: bool,
}

impl Family {
    /// この書体でその言語の字が組めるか。
    pub fn covers(&self, s: Script) -> bool {
        match s {
            Script::Japanese => self.japanese,
            Script::Korean => self.hangul,
            // **簡体と繁体は字の有無では分けられません。** Noto の SC は
            // 繁体の字も持っていますし、その逆もあります。どちらを使うかは
            // 下の名前の一覧で決めて、ここは漢字が組めるかだけを見ます
            Script::SimplifiedChinese | Script::TraditionalChinese => self.han,
            Script::Cyrillic => self.cyrillic,
            Script::Vietnamese => self.vietnamese,
            Script::Latin => self.latin,
        }
    }
}

/// 標準の書体を選ぶときの、言語のまとまり。
///
/// **標準の書体は OS と言語で変わります**(2026-08-26 発注者)。同じ
/// 「何も指定していない文書」でも、日本語の人には日本語の書体を、韓国語の
/// 人にはハングルの書体を出さないと豆腐になります。OS でも変わるのは、
/// その OS の人が見慣れた書体が違うからです(Windows は游ゴシック、
/// Mac はヒラギノ、Linux は Noto)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Japanese,
    Korean,
    SimplifiedChinese,
    TraditionalChinese,
    Cyrillic,
    Vietnamese,
    Latin,
}

/// 画面の言語の札から、書体を選ぶときのまとまりを決めます。
/// 知らない札はラテン文字として扱います。
pub fn script_of(lang: &str) -> Script {
    match lang.split('-').next().unwrap_or(lang) {
        "ja" => Script::Japanese,
        "ko" => Script::Korean,
        "zh" => {
            // zh-tw と zh-hk は繁体、ただの zh は簡体
            if lang.eq_ignore_ascii_case("zh-tw") || lang.eq_ignore_ascii_case("zh-hk") {
                Script::TraditionalChinese
            } else {
                Script::SimplifiedChinese
            }
        }
        "ru" | "uk" | "bg" | "sr" => Script::Cyrillic,
        "vi" => Script::Vietnamese,
        _ => Script::Latin,
    }
}

/// 探す場所。
///
/// **Linux では置き場を決め打ちにしません**(2026-08-28 発注者「書体の
/// 置き場が決め打ちというのはおかしい」)。どこに書体があるかを決めるのは
/// fontconfig で、それは配布版や利用者の設定で変わります。この機械でも
/// `/usr/share/texmf/…` の下に 4 か所あり、決め打ちでは拾えていませんでした。
///
/// 順に、fontconfig の設定 → OS ごとの既定 → 利用者の置き場 →
/// `OFFICE_FONT_DIR` を見ます。同じ所は後で1回に畳みます。
fn dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fontconfig_dirs();
    // OS ごとの既定。**fontconfig の無い機種のための物**です。
    // /system/fonts は Android(Noto CJK が標準で居る)。iOS は mac と同じ
    // /System/Library/Fonts。スマホは wheel(PEP 730/738)で Python から
    // エンジンを使う道があるので、探索先に最初から入れておく(2026-08-13)
    v.extend(
        ["/usr/share/fonts", "/usr/local/share/fonts",
         "/Library/Fonts", "/System/Library/Fonts", "/system/fonts",
         "C:\\Windows\\Fonts"]
            .iter()
            .map(PathBuf::from),
    );
    if let Ok(h) = std::env::var("HOME") {
        v.push(PathBuf::from(&h).join(".fonts"));
        v.push(PathBuf::from(&h).join(".local/share/fonts"));
        v.push(PathBuf::from(&h).join("Library/Fonts"));
    }
    // XDG の置き場(`XDG_DATA_DIRS` は `:` 区切り)
    for base in xdg_data_dirs() {
        v.push(base.join("fonts"));
    }
    if let Ok(d) = std::env::var("OFFICE_FONT_DIR") {
        v.push(PathBuf::from(d));
    }
    // **同じ所を2度走査しない。** 走査は再帰なので、重なると目に見えて遅くなります
    let mut mita = std::collections::HashSet::new();
    v.retain(|p| mita.insert(p.clone()));
    v
}

/// `XDG_DATA_HOME` と `XDG_DATA_DIRS`(既定は `~/.local/share` と
/// `/usr/local/share:/usr/share`)
fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    match std::env::var("XDG_DATA_HOME") {
        Ok(d) if !d.is_empty() => v.push(PathBuf::from(d)),
        _ => {
            if let Ok(h) = std::env::var("HOME") {
                v.push(PathBuf::from(h).join(".local/share"));
            }
        }
    }
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    v.extend(dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v
}

/// **fontconfig の設定に書いてある置き場。**
///
/// `<dir>` の行だけを見ます。XML の解釈はしません — 置き場を知りたい
/// だけなので、`<dir…>…</dir>` を拾えば足ります。`prefix="xdg"` は
/// `XDG_DATA_HOME` からの相対、頭の `~` は `HOME` です。
///
/// 設定が無い機械(mac・Windows・Android)では空を返し、下の既定に任せます。
fn fontconfig_dirs() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![PathBuf::from("/etc/fonts/fonts.conf")];
    // conf.d の断片。配布版が texlive などの置き場をここで足します
    if let Ok(rd) = std::fs::read_dir("/etc/fonts/conf.d") {
        let mut kake: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "conf"))
            .collect();
        kake.sort();
        files.extend(kake);
    }
    if let Ok(h) = std::env::var("HOME") {
        let cfg = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&h).join(".config"));
        files.push(cfg.join("fontconfig/fonts.conf"));
        files.push(PathBuf::from(&h).join(".fonts.conf"));
    }

    let mut out = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (attr, body) in dir_tags(&text) {
            let body = body.trim();
            if body.is_empty() {
                continue;
            }
            let p = if attr.contains(r#"prefix="xdg""#) {
                let Some(base) = xdg_data_dirs().into_iter().next() else { continue };
                base.join(body)
            } else if let Some(rest) = body.strip_prefix("~/") {
                let Ok(h) = std::env::var("HOME") else { continue };
                PathBuf::from(h).join(rest)
            } else {
                PathBuf::from(body)
            };
            out.push(p);
        }
    }
    out
}

/// `<dir …>…</dir>` を (属性, 中身) で拾う
fn dir_tags(text: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("<dir") {
        rest = &rest[i + 4..];
        // `<dirs>` のような別の名前は飛ばす
        if !rest.starts_with(|c: char| c == '>' || c.is_whitespace()) {
            continue;
        }
        let Some(j) = rest.find('>') else { break };
        let attr = &rest[..j];
        rest = &rest[j + 1..];
        let Some(k) = rest.find("</dir>") else { break };
        out.push((attr, &rest[..k]));
        rest = &rest[k + 6..];
    }
    out
}

/// この機械にあるフォント。**リボンのフォント一覧はこれ。**
///
/// 名前はファイル名ではなく、フォント自身が持っている書体名から取る
/// (`ipaexg.ttf` ではなく「IPAexゴシック」と出す)。
pub fn list() -> &'static [Family] {
    static CACHE: OnceLock<Vec<Family>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut out: Vec<Family> = Vec::new();
        for d in dirs() {
            scan(&d, &mut out, 0);
        }
        // 同じ書体名の中では**素の字面を先に**。
        // 並び順で先頭を採ると「BIZ UDPゴシック」を頼んで Bold が返る
        out.sort_by(|a, b| a.name.cmp(&b.name).then(b.regular.cmp(&a.regular)));
        out.dedup_by(|a, b| a.name == b.name);
        out
    })
}

fn scan(dir: &Path, out: &mut Vec<Family>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan(&p, out, depth + 1);
            continue;
        }
        let ext = p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase());
        if !matches!(ext.as_deref(), Some("ttf") | Some("otf") | Some("ttc")) {
            continue;
        }
        let Ok(data) = std::fs::read(&p) else { continue };
        let n = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        for i in 0..n {
            if let Some(f) = read_family(&data, i, &p) {
                out.push(f);
            }
        }
    }
}

/// フォント自身が名乗っている書体名を読む。
fn read_family(data: &[u8], index: u32, path: &Path) -> Option<Family> {
    let face = ttf_parser::Face::parse(data, index).ok()?;
    // name_id 1 = 書体名。日本語名があればそちらを採る(画面に出すのは人が読む名前)
    let mut ascii: Option<String> = None;
    let mut local: Option<String> = None;
    for n in face.names() {
        if n.name_id != 1 {
            continue;
        }
        let Some(s) = n.to_string() else { continue };
        if s.is_empty() {
            continue;
        }
        if s.is_ascii() {
            ascii.get_or_insert(s);
        } else {
            local.get_or_insert(s);
        }
    }
    // 画面に出すのは人が読む名前(日本語名があればそちら)。**英語名も捨てない** —
    // 絞り込みで「Yu Gothic」と打っても「游ゴシック」に当てるため、副名として持つ
    let name = local.clone().or_else(|| ascii.clone())?;
    let ascii_name = ascii.unwrap_or_else(|| name.clone());
    // どの言語の字を組めるか。**代表の1字が引ければ足りる**とみなします。
    // 全部の字を数えると走査が重くなりますし、この判定は「標準の書体を
    // 選ぶ」ためだけに使う物なので、それで足ります
    let han = face.glyph_index('日').is_some();
    let japanese = face.glyph_index('あ').is_some() && han;
    let hangul = face.glyph_index('한').is_some();
    let cyrillic = face.glyph_index('Ж').is_some();
    let latin = face.glyph_index('A').is_some() && face.glyph_index('é').is_some();
    let vietnamese = latin && face.glyph_index('ế').is_some();
    // **「標準」だけでは足りません。** BIZ UD ゴシックの Bold は
    // OS/2 の標準の旗も立てていて(2026-08-28 に実物で確認)、標準の顔と
    // 見分けが付きません。太字と斜体でないことも見ます
    let regular = face.is_regular() && !face.is_bold() && !face.is_italic();
    Some(Family {
        name,
        ascii: ascii_name,
        path: path.to_path_buf(),
        index,
        japanese,
        han,
        hangul,
        cyrillic,
        latin,
        vietnamese,
        regular,
    })
}

/// 名前から実体を引く。**文書が指定した書体を出すための道。**
///
/// 完全一致で見つからなければ、大文字小文字と空白を無視して探す
/// (「MS ゴシック」「MSゴシック」の揺れを吸う)。
pub fn resolve(name: &str) -> Option<&'static Family> {
    let all = list();
    if let Some(f) = all.iter().find(|f| f.name == name || f.ascii == name) {
        return Some(f);
    }
    let key = norm(name);
    all.iter().find(|f| norm(&f.name) == key || norm(&f.ascii) == key)
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '　' && *c != '-' && *c != '_')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 書体の系統。**明朝の書類を黙ってゴシックにしない**ための区別です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generic {
    /// 明朝・セリフ(縦横に太さの差があり、端に飾りがある)
    Serif,
    /// ゴシック・サンセリフ(太さが一定で、飾りが無い)
    SansSerif,
}

/// 書体の名前から系統を読む。どちらとも読めなければ `None`。
pub fn read_generic(name: &str) -> Option<Generic> {
    let lower = name.to_lowercase();
    // 名前に系統が書いてある物(明朝/ゴシック/serif/sans)を先に見ます
    if name.contains("明朝") || lower.contains("mincho") || lower.contains("serif") {
        // "sans serif" は "serif" を含むので、先にサンセリフを外します
        if !lower.contains("sans") {
            return Some(Generic::Serif);
        }
    }
    if name.contains("ゴシック")
        || lower.contains("gothic")
        || lower.contains("sans")
        || name.contains("メイリオ")
        || lower.contains("meiryo")
        || name.contains("黑")
        || name.contains("黒")
        || name.contains("고딕")
    {
        return Some(Generic::SansSerif);
    }
    // 名前に書いていない物は、よく使われる書体を名指しで覚えます。
    // Windows の docx が名乗るのはたいていこの辺りです
    const SERIF_NAMES: &[&str] = &[
        "times new roman", "times", "georgia", "garamond", "book antiqua",
        "palatino", "cambria", "constantia", "songti", "宋体", "新細明體",
        "batang", "바탕",
    ];
    const SANS_NAMES: &[&str] = &[
        "arial", "helvetica", "calibri", "aptos", "segoe ui", "verdana",
        "tahoma", "candara", "corbel", "roboto", "gulim", "굴림",
    ];
    if SERIF_NAMES.iter().any(|n| lower.contains(n)) {
        return Some(Generic::Serif);
    }
    if SANS_NAMES.iter().any(|n| lower.contains(n)) {
        return Some(Generic::SansSerif);
    }
    None
}

/// 無い書体の筋の通った代替。**明朝の書類を黙ってゴシックにしない。**
///
/// Windows の書体(ＭＳ 明朝、Times New Roman など)は Linux に無いのが
/// 普通なので、系統(明朝/ゴシック)を保って置き換えます。
///
/// どの一覧から選ぶかは**画面の言語**で変わります(2026-08-26)。日本語の
/// 一覧しか持っていなかったので、ドイツ語の画面で Times New Roman の
/// 文書を開くと日本語の明朝になっていました。
pub fn substitute(name: &str) -> Option<&'static Family> {
    let k = read_generic(name)?;
    // 並びは「入れた書体(IPA/Noto)→ OS の持ち物」。後半は実機の書体 —
    // Mac は Hiragino、Windows は游/メイリオ/ＭＳ が標準で、ここに無いと
    // Noto も IPA も入れていない実機で明朝がゴシックの fallback に落ちる
    // (2026-08-13、CI の3 OS 化で気づいた製品側の穴)。
    // 書体は日本語名と英語名の両方で名乗ることがあるので、両方書く
    // (resolve は空白・大小文字の揺れは吸うが、言語までは翻訳しない)
    let candidates: &[&str] = match (script_of(&default_language()), k) {
        (Script::Japanese, Generic::Serif) => &[
            "IPAex明朝", "Noto Serif CJK JP", "BIZ UDP明朝", "BIZ UD明朝", "IPA P明朝", "IPA明朝",
            "ヒラギノ明朝 ProN", "Hiragino Mincho ProN",
            "游明朝", "游明朝体", "Yu Mincho", "ＭＳ 明朝", "MS Mincho",
        ],
        (Script::Japanese, Generic::SansSerif) => &[
            "IPAexゴシック", "Noto Sans CJK JP", "BIZ UDPゴシック", "IPA Pゴシック",
            "ヒラギノ角ゴシック", "Hiragino Sans", "Hiragino Kaku Gothic ProN",
            "游ゴシック", "Yu Gothic", "メイリオ", "Meiryo", "ＭＳ ゴシック", "MS Gothic",
        ],
        (Script::Korean, Generic::Serif) => {
            &["Noto Serif CJK KR", "NanumMyeongjo", "나눔명조", "바탕", "Batang"]
        }
        (Script::Korean, Generic::SansSerif) => &[
            "Noto Sans CJK KR", "NanumGothic", "나눔고딕",
            "Apple SD Gothic Neo", "맑은 고딕", "Malgun Gothic",
        ],
        (Script::SimplifiedChinese, Generic::Serif) => {
            &["Noto Serif CJK SC", "Source Han Serif SC", "宋体", "SimSun", "STSong"]
        }
        (Script::SimplifiedChinese, Generic::SansSerif) => &[
            "Noto Sans CJK SC", "Source Han Sans SC", "WenQuanYi Micro Hei",
            "PingFang SC", "微软雅黑", "Microsoft YaHei",
        ],
        (Script::TraditionalChinese, Generic::Serif) => {
            &["Noto Serif CJK TC", "Source Han Serif TC", "新細明體", "PMingLiU"]
        }
        (Script::TraditionalChinese, Generic::SansSerif) => &[
            "Noto Sans CJK TC", "Source Han Sans TC",
            "PingFang TC", "微軟正黑體", "Microsoft JhengHei",
        ],
        (_, Generic::Serif) => &[
            "Liberation Serif", "DejaVu Serif", "Noto Serif", "Nimbus Roman",
            "Times New Roman", "Georgia", "Cambria",
        ],
        (_, Generic::SansSerif) => &[
            "Liberation Sans", "DejaVu Sans", "Noto Sans", "Nimbus Sans",
            "Helvetica Neue", "Helvetica", "Arial", "Calibri", "Segoe UI",
        ],
    };
    candidates.iter().find_map(|c| resolve(c))
}

/// 標準の書体の候補を、**この OS で見慣れた順に**並べたもの。
///
/// 先に来た名前から探して、この機械にある最初の1つを使います。名前が
/// 1つも当たらなければ、その言語の字が組める書体から選びます。
///
/// 名前を日本語と英語の両方で書いてあるのは、書体が OS の言語によって
/// 違う名前を名乗るからです([`resolve`] は空白や大小文字の揺れは吸い
/// ますが、名前の翻訳まではしません)。
///
/// ここに並ぶのは**ゴシック(サンセリフ)だけ**です。明朝(セリフ)は
/// [`serif_cands`] にあります。
fn default_cands(s: Script) -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    match s {
        Script::Japanese => &["游ゴシック", "Yu Gothic", "メイリオ", "Meiryo", "ＭＳ Ｐゴシック", "MS PGothic"],
        Script::Korean => &["맑은 고딕", "Malgun Gothic", "굴림", "Gulim"],
        Script::SimplifiedChinese => &["微软雅黑", "Microsoft YaHei", "宋体", "SimSun"],
        Script::TraditionalChinese => &["微軟正黑體", "Microsoft JhengHei", "新細明體", "PMingLiU"],
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Calibri", "Segoe UI", "Arial", "Times New Roman"]
        }
    }
    #[cfg(target_os = "macos")]
    match s {
        Script::Japanese => &["ヒラギノ角ゴシック", "Hiragino Sans", "Hiragino Kaku Gothic ProN"],
        Script::Korean => &["Apple SD Gothic Neo", "애플 SD 산돌고딕 Neo", "AppleGothic"],
        Script::SimplifiedChinese => &["PingFang SC", "苹方-简", "Heiti SC", "STHeiti"],
        Script::TraditionalChinese => &["PingFang TC", "蘋方-繁", "Heiti TC"],
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Helvetica Neue", "Helvetica", "Arial", "Lucida Grande"]
        }
    }
    // Linux と、その他(Android は Noto が標準で居ます)
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    match s {
        Script::Japanese => &["Noto Sans CJK JP", "IPAexゴシック", "BIZ UDPゴシック", "IPA Pゴシック"],
        Script::Korean => &["Noto Sans CJK KR", "NanumGothic", "나눔고딕", "UnDotum"],
        Script::SimplifiedChinese => &["Noto Sans CJK SC", "Source Han Sans SC", "WenQuanYi Micro Hei"],
        Script::TraditionalChinese => &["Noto Sans CJK TC", "Source Han Sans TC", "WenQuanYi Micro Hei"],
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Liberation Sans", "DejaVu Sans", "Noto Sans", "Nimbus Sans"]
        }
    }
}

/// **明朝(セリフ)の候補**を、言語ごとに並べたもの。
///
/// [`default_cands`] と同じ役ですが、そちらはゴシックしか並んでいません。
/// 2026-08-30 まで明朝の候補が無く、本文の書体は必ず全体の走査に落ちて
/// いました。走査は「ラテン文字が組める明朝の先頭」を拾うので、英語の
/// 文書の本文に **BIZ UDP明朝**(日本語の書体)が入っていました。中国語
/// と韓国語も、その言語の書体ではない物が入っていました。
fn serif_cands(s: Script) -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    match s {
        Script::Japanese => &["游明朝", "Yu Mincho", "ＭＳ Ｐ明朝", "MS PMincho"],
        Script::Korean => &["바탕", "Batang", "궁서", "Gungsuh"],
        Script::SimplifiedChinese => &["宋体", "SimSun", "新宋体", "NSimSun"],
        Script::TraditionalChinese => &["新細明體", "PMingLiU", "細明體", "MingLiU"],
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Times New Roman", "Georgia", "Cambria"]
        }
    }
    #[cfg(target_os = "macos")]
    match s {
        Script::Japanese => &["ヒラギノ明朝 ProN", "Hiragino Mincho ProN", "YuMincho"],
        Script::Korean => &["AppleMyungjo", "Apple SD Gothic Neo"],
        Script::SimplifiedChinese => &["Songti SC", "宋体-简", "STSong"],
        Script::TraditionalChinese => &["Songti TC", "宋体-繁", "LiSong Pro"],
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Times New Roman", "Georgia", "Palatino"]
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    match s {
        Script::Japanese => &["Noto Serif CJK JP", "IPAex明朝", "BIZ UDP明朝", "IPA P明朝"],
        Script::Korean => &["Noto Serif CJK KR", "Baekmuk Batang", "은 바탕"],
        Script::SimplifiedChinese => &["Noto Serif CJK SC", "Source Han Serif SC", "AR PL SungtiL GB"],
        Script::TraditionalChinese => {
            &["Noto Serif CJK TC", "Source Han Serif TC", "AR PL Mingti2L Big5"]
        }
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            &["Liberation Serif", "DejaVu Serif", "Noto Serif", "Nimbus Roman"]
        }
    }
}

/// **その言語の標準の書体。** 文書もテンプレートも書体を言っていないとき、
/// これを使います。
///
/// 日本語の書体を決め打ちにしていた時期がありますが、それは誤りでした
/// (2026-08-26 発注者「標準フォントは、os と言語によって変えないと
/// いけない」)。韓国語の画面で日本語の書体を出すと、ハングルが豆腐に
/// なります。
/// **役ごとの既定の書体**(見出しはゴシック、本文は明朝)。
///
/// 文書は役で書体を変えます(2026-08-26 発注者「タイトルはゴシック、
/// 本文は明朝、コードは等幅」)。[`default_family`] は役を持たない
/// 「その言語の既定」なので、役が要るときはこちらを使います。
///
/// 機械に無ければ `None`。呼ぶ側が名前を決めます。
pub fn default_generic(lang: &str, g: Generic) -> Option<&'static Family> {
    let s = script_of(lang);
    // 役に合う候補の一覧から探します。明朝を頼まれてゴシックの一覧を
    // 引いても当たらないので、必ず全体の走査に落ちていました(2026-08-30)
    let cands: &[&str] = match g {
        Generic::Serif => serif_cands(s),
        _ => default_cands(s),
    };
    for c in cands {
        if let Some(f) = resolve(c) {
            if read_generic(&f.name) == Some(g) {
                return Some(f);
            }
        }
    }
    // 候補に無ければ、その言語の字が組める物から系統で選びます。
    //
    // **その言語の書体を先に見ます。** ラテン文字は日本語の書体でも
    // 組めてしまうので、ただ走査すると英語の文書の本文が BIZ UDP明朝に
    // なりました。日本語・中国語・韓国語の書体でない物を先に見ます
    let yoso = |f: &&Family| match s {
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            f.japanese || f.han || f.hangul
        }
        _ => false,
    };
    let au = |f: &&Family| f.covers(s) && read_generic(&f.name) == Some(g);
    list()
        .iter()
        .find(|f| au(f) && f.regular && !yoso(f))
        .or_else(|| list().iter().find(|f| au(f) && !yoso(f)))
        .or_else(|| list().iter().find(|f| au(f) && f.regular))
        .or_else(|| list().iter().find(au))
}

pub fn default_family(lang: &str) -> Option<&'static Family> {
    let s = script_of(lang);
    for c in default_cands(s) {
        if let Some(f) = resolve(c) {
            return Some(f);
        }
    }
    // 名前で当たらないときは、**その言語の字が組める書体**から。
    // 素の字面を先に見るのは、太字だけ入っている機械で全部太字に
    // ならないためです
    list()
        .iter()
        .find(|f| f.regular && f.covers(s))
        .or_else(|| list().iter().find(|f| f.covers(s)))
        // その言語の字が1つも無ければ、せめて字が出る物を返します。
        // 豆腐にはなりますが、何も出ないよりは直す手がかりになります
        .or_else(|| list().iter().find(|f| f.regular))
}

/// 文書が書体を指定していないとき、あるいは指定されたものが無いときに使う。
/// 言語は [`set_default_language`] で入れた物を見ます。
pub fn fallback() -> Option<&'static Family> {
    default_family(&default_language())
}

/// 標準の書体を選ぶときに見る言語。入れていなければ `book::lang` が決めます。
static UI_LANG: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// **既定の言語は処理系に1つしかありません。**
///
/// 試験を並べて回すと、言語を入れ替える試験の値が、書体を見る試験にも
/// 届いてしまいます。入れ替える側も見る側も、この錠を取ってから動きます
/// (2026-08-27 に、回すたびに違う試験が落ちるので気づきました)。
/// **`#[cfg(test)]` にはできません。** それは crate の中だけの印で、
/// 取りたいのは別の crate(`ooxml`)の試験だからです。用紙が言語で変わる
/// ようになった 2026-08-30 から、`ooxml` の試験も取ります
/// (`lang::i18n::LANG_LOCK` と同じ形)
pub static LANG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 錠を取ります。前の試験が落ちて錠が壊れていても、そのまま使います
pub fn lang_lock() -> std::sync::MutexGuard<'static, ()> {
    LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// **画面の言語をエンジンに渡す。**
///
/// エンジンは設定ファイルを読みません(画面にも設定にも依存しないのが
/// このクレートの決まりです)。なので、言語を知っている側から入れます。
/// 呼ばなければ `book::lang::decide` が決めます(環境変数 → 設定 → OS →
/// en)。言語を変えたときも、もう一度呼んでください。
pub fn set_default_language(tag: &str) {
    *UI_LANG.write().expect("言語の錠") = Some(tag.to_string());
}

/// いまエンジンが見ている言語。
pub fn default_language() -> String {
    if let Some(l) = UI_LANG.read().expect("言語の錠").clone() {
        return l;
    }
    // **決め方は book に1本あります**(環境変数 → 設定 → OS → en)。
    // 2026-08-30 まではここが `ja` の決め打ちで、設定も OS も見ていません
    // でした。Python から使うと、ドイツ語の設定にしてある機械でも
    // 日本語の既定で組まれます
    book::lang::decide(None)
}

/// 文書の指定を実体に結び付ける。
///
/// 指定された書体が無ければ、**似た書体に置き換えて刷ります**。エラーには
/// しません(2026-08-29 発注者。止まると実用になりません)。返り値の2つめが
/// `false` なら置き換えた印です。画面のように利用者へ伝えられる所では、
/// 置き換えたことを出してください。
pub fn for_document(wanted: Option<&str>) -> Result<(&'static Family, bool), String> {
    if let Some(w) = wanted.filter(|s| !s.is_empty()) {
        if let Some(f) = resolve(w) {
            return Ok((f, true));
        }
        // 系統を保った代替(明朝→明朝)を先に。無ければ一般の代替
        if let Some(f) = substitute(w) {
            return Ok((f, false));
        }
        let f = fallback().ok_or_else(|| missing(Some(w)))?;
        return Ok((f, false));
    }
    fallback().map(|f| (f, true)).ok_or_else(|| missing(None))
}

/// **その文書に出てくる字を、全部組める書体を選ぶ。**
///
/// [`for_document`] は言語と名前だけで選びます。それだと、英語の設定の
/// 機械でドイツ語の文書に日本語を1行入れたとき、選ばれた書体が仮名を
/// 持っておらず、**その字だけ紙から消えます**(PDF は字形が無い字を
/// 空白として刷ります)。2026-08-30 に、既定の言語を en にしたときに
/// 出てきました。
///
/// 名前の指定は今までどおり効きます。指定された書体が文中の字を組める
/// なら、そのまま使います。組めない字があるときだけ、組める書体に
/// 換えます。返り値の2つめは [`for_document`] と同じで、指定どおりなら
/// `true` です。
pub fn for_text(
    wanted: Option<&str>,
    text: impl Iterator<Item = char>,
) -> Result<(&'static Family, bool), String> {
    let need = scripts_in(text);
    let (f, exact) = for_document(wanted)?;
    if need.iter().all(|s| f.covers(*s)) {
        return Ok((f, exact));
    }
    // 系統(明朝・ゴシック・等幅)は保ったまま、字が足りる物を探します
    let g = read_generic(&f.name);
    let ok = |c: &&Family| need.iter().all(|s| c.covers(*s));
    let pick = list()
        .iter()
        .find(|c| c.regular && read_generic(&c.name) == g && ok(c))
        .or_else(|| list().iter().find(|c| c.regular && ok(c)))
        .or_else(|| list().iter().find(ok));
    Ok(match pick {
        // 換えたので、指定どおりではありません
        Some(c) => (c, false),
        // 全部組める書体が1つも無ければ、元の選択のままにします。
        // 一部が消えますが、勝手に別の言語の書体へ寄せるよりはましです
        None => (f, exact),
    })
}

/// 文中に出てくる字から、要る文字の種類を数え上げます。
///
/// ラテン文字は、記号の付かない ASCII ならどの書体でも組めるので数えません。
/// 記号つき(ä é)から先を [`Script::Latin`] として数えます。
fn scripts_in(text: impl Iterator<Item = char>) -> Vec<Script> {
    let mut v: Vec<Script> = Vec::new();
    let mut add = |s: Script| {
        if !v.contains(&s) {
            v.push(s);
        }
    };
    for c in text {
        match c as u32 {
            0x00..=0x7f => {}
            0x80..=0x24f => add(Script::Latin),
            0x400..=0x52f => add(Script::Cyrillic),
            // ベトナム語だけに出る、記号が二重に付く字
            0x1e00..=0x1eff => add(Script::Vietnamese),
            // 仮名。**漢字より先に見ます** — 仮名があれば日本語です
            0x3040..=0x30ff => add(Script::Japanese),
            0xac00..=0xd7af | 0x1100..=0x11ff => add(Script::Korean),
            // 漢字。日本語か中国語かは字では分かれないので、漢字が
            // 組めることだけを求めます
            0x3400..=0x9fff | 0xf900..=0xfaff => add(Script::SimplifiedChinese),
            _ => {}
        }
    }
    v
}

fn missing(wanted: Option<&str>) -> String {
    // **入れてもらう物の名前は言語で変わります。** 韓国語の人に
    // 「fonts-ipaexfont を入れてください」と言っても直りません
    let to_insert = match script_of(&default_language()) {
        Script::Japanese => "fonts-noto-cjk か fonts-ipaexfont",
        Script::Korean => "fonts-noto-cjk か fonts-nanum",
        Script::SimplifiedChinese | Script::TraditionalChinese => "fonts-noto-cjk",
        Script::Cyrillic | Script::Vietnamese | Script::Latin => {
            "fonts-liberation か fonts-dejavu"
        }
    };
    match wanted {
        Some(w) => format!(
            "書体「{w}」がこの機械にありません。代わりに使える書体も見つかりません\
             ({to_insert} を入れてください)"
        ),
        None => format!("使える書体が見つかりません({to_insert} を入れてください)"),
    }
}

/// 実体を読む。
pub fn load(f: &Family) -> Result<Vec<u8>, String> {
    std::fs::read(&f.path).map_err(|e| format!("{}: {e}", f.path.display()))
}


/// **コードの塊に使う等幅の書体**を、この機械から探す。
///
/// 日本語の入るコードもあるので、和文を持つ物から順に見ます。
/// どれも入っていなければ `None` — *代わりの書体で等幅のふりはしません*。
/// 等幅でない字で組むと桁が揃わず、かえって読みにくくなります。
pub fn monospace() -> Option<&'static Family> {
    const CANDS: &[&str] = &[
        "Noto Sans Mono CJK JP",   // Linux の既定の組み合わせ
        "BIZ UDGothic",            // Windows(等幅の和文)
        "MS Gothic",
        "Osaka-Mono",              // Mac
        "IPAGothic",
        "Noto Sans Mono",          // 和文が無い物は最後
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Courier New",
    ];
    CANDS.iter().find_map(|n| resolve(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_count_the_fonts_on_this_machine() {
        let all = list();
        assert!(!all.is_empty(), "1つも見つからない");
        assert!(all.iter().any(|f| f.japanese), "日本語が組めるものが無い");
    }

    #[test]
    fn the_name_is_the_typeface_not_the_file() {
        // 「ipaexg」ではなく「IPAexゴシック」、「NotoSansCJK-Regular」ではなく
        // 「Noto Sans CJK JP」と名乗らせる。
        //
        // **特定の書体を要求しない。** 以前は IPAexゴシックを expect していたが、
        // それはこの機械に入っていただけで、試験したい性質(名前を name テーブル
        // から取っているか)とは関係がない。CI(fonts-noto-cjk しか無い)で落ちた
        // ので、**入っている書体のどれかで**同じ性質を見る形に改めた(2026-08-10)。
        // 隣の 同名なら素の字面を採る が最初からこの作法だった
        let all = list();
        let witness = all
            .iter()
            .find(|f| f.japanese && f.path.file_stem().is_some_and(|s| s.to_string_lossy() != f.name));
        let f = witness.expect(
            "日本語の書体が1つも無いか、どれもファイル名をそのまま書体名にしている\
             (fonts-noto-cjk か fonts-ipaexfont を入れてください)",
        );
        assert!(f.japanese);
        // 名前に拡張子や連番が混じっていないこと(ファイル名を切っただけの疑い)
        assert!(!f.name.contains(".ttf") && !f.name.contains(".otf") && !f.name.contains(".ttc"),
            "書体名にファイルの拡張子が混じっている: {}", f.name);
    }

    #[test]
    fn for_the_same_name_the_regular_face_wins() {
        // 「BIZ UDPゴシック」を頼んで Bold が返ってはいけない
        for name in ["BIZ UDPゴシック", "IPAexゴシック", "Noto Sans CJK JP"] {
            if let Some(f) = resolve(name) {
                assert!(f.regular, "{name} で太字・斜体を返した: {}", f.path.display());
            }
        }
    }

    #[test]
    fn a_name_resolves_to_a_real_font() {
        let first = list().iter().find(|f| f.japanese).unwrap();
        let got = resolve(&first.name).expect("引けない");
        assert_eq!(got.path, first.path);
    }

    #[test]
    fn absorbs_spelling_variation() {
        let first = list().iter().find(|f| f.japanese).unwrap();
        let messy = first.name.to_uppercase().replace(' ', "");
        assert!(resolve(&messy).is_some(), "「{}」を引けない", messy);
    }

    #[test]
    fn a_missing_font_is_substituted_but_reported() {
        let _lang = lang_lock();
        // 落ち先はいまの言語の既定なので、言語を明示します(2026-08-30)
        set_default_language("ja");
        let (f, exact) = for_document(Some("存在しない書体XYZ")).unwrap();
        assert!(!exact, "指定と違う書体なのに、合っていることにした");
        assert!(f.japanese, "英字フォントに落ちている(豆腐になる)");
        *UI_LANG.write().unwrap() = None;
    }

    /// **英語の設定でも、日本語の字は消えない。**
    ///
    /// 2026-08-30 に既定の言語を en にしたとき、英語の既定の書体
    /// (Liberation Sans)には仮名が無いので、PDF から「見本」の2字だけが
    /// 消えました。文中の字を見て選び直します。
    #[test]
    fn japanese_in_an_english_document_still_gets_a_font_that_has_it() {
        let _lang = lang_lock();
        set_default_language("en");
        let (plain, _) = for_text(None, "Hello".chars()).unwrap();
        assert!(!plain.japanese, "英語だけの文書に日本語の書体を選んだ");
        let (mixed, _) = for_text(None, "Hello 見本".chars()).unwrap();
        assert!(mixed.japanese, "仮名と漢字が組めない書体のままだった");
        *UI_LANG.write().unwrap() = None;
    }

    /// **本文の書体は、その言語の書体から選ぶ。**
    ///
    /// 2026-08-30 まで明朝の候補の一覧が無く、本文は必ず全体の走査に
    /// 落ちていました。走査は「ラテン文字が組める明朝の先頭」を拾うので、
    /// 英語の文書の本文に BIZ UDP明朝(日本語の書体)が入っていました。
    /// 手引きの「英語は本文セリフ」と食い違っていた所です。
    #[test]
    fn the_body_font_comes_from_the_language_not_from_japanese() {
        let _lang = lang_lock();
        for lang in ["en", "de", "pt-br", "vi", "ru"] {
            let f = default_generic(lang, Generic::Serif)
                .unwrap_or_else(|| panic!("{lang}: 明朝が見つからない"));
            assert!(
                !f.japanese,
                "{lang} の本文に日本語の書体が入った: {}",
                f.name
            );
            assert_eq!(read_generic(&f.name), Some(Generic::Serif), "{lang}");
        }
        // 見出し(ゴシック)も同じこと
        for lang in ["en", "de", "vi"] {
            let f = default_generic(lang, Generic::SansSerif).expect("ゴシック");
            assert!(!f.japanese, "{lang} の見出しに日本語の書体が入った: {}", f.name);
        }
    }

    /// 指定した書体でその字が組めるなら、換えません
    #[test]
    fn a_font_that_can_set_the_text_is_left_alone() {
        let _lang = lang_lock();
        set_default_language("ja");
        let (f, exact) = for_text(None, "日本語のみ".chars()).unwrap();
        assert!(exact, "換える要が無いのに換えた");
        assert!(f.japanese);
        *UI_LANG.write().unwrap() = None;
    }

    #[test]
    fn with_no_request_it_picks_a_font_that_can_set_japanese() {
        let _lang = lang_lock();
        // **言語を明示します。** 2026-08-30 に既定が en になり、ここを
        // 環境まかせにすると、英語の設定の機械で落ちるようになりました
        set_default_language("ja");
        let (f, exact) = for_document(None).unwrap();
        assert!(exact);
        assert!(f.japanese);
        *UI_LANG.write().unwrap() = None;
    }

    #[test]
    fn a_serif_document_does_not_turn_sans() {
        let _lang = lang_lock();
        // ＭＳ 明朝は Linux に無い。でも代替は明朝系であるべき
        let (f, exact) = for_document(Some("ＭＳ 明朝")).unwrap();
        assert!(!exact);
        assert!(
            f.name.contains("明朝") || f.name.contains("Serif"),
            "明朝の代替がゴシックになった: {}",
            f.name
        );
        let (g, _) = for_document(Some("ＭＳ ゴシック")).unwrap();
        assert!(
            g.name.contains("ゴシック") || g.name.contains("Sans"),
            "ゴシックの代替が変: {}",
            g.name
        );
    }

    #[test]
    fn the_resolved_font_can_typeset() {
        let _lang = lang_lock();
        // 「日」の幅を見るので、日本語で選びます(2026-08-30 に既定が
        // en になり、環境まかせだと英字の書体が返ってきます)
        set_default_language("ja");
        let (f, _) = for_document(None).unwrap();
        let data = load(f).unwrap();
        let m = crate::Metrics::new(&data).expect("解釈できない");
        let w = m.advance_mm('日', 10.5);
        assert!(w > 3.0 && w < 4.5, "全角の幅がおかしい: {w}mm ({})", f.name);
        *UI_LANG.write().unwrap() = None;
    }
}
