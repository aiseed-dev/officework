//! **フォルダから探す** — 指定した場所を辿って、複数のファイルを串刺しで探す。
//!
//! 発注者 2026-08-17(SFIND の写真つき): 「同じディレクトリを指定しての検索を
//! ファイルメニューに組み込めますか」。
//!
//! 素の字(.txt / .md / .adoc / .py …)はここが読む。**文書の中身**
//! (.docx の本文・.xlsx のセル)は呼ぶ側が [`Query::extract`] で渡す —
//! face は ooxml も sheet も持たない(持ち運べる層のまま)。
//! これで **一度 txt に落としてから探す手間が消える**(写真の運用の上積み)。
//!
//! # 決め
//!
//! - **見つからない物は黙って飛ばさない。** 読めなかったファイルは数え、
//!   呼ぶ側が「何件見なかったか」を言えるようにする
//! - 上限つき(ファイル数・当たり数)。**打ち切ったらそう言う** —
//!   途中で止めたのに全部見たように見せない
//! - 隠しフォルダ(`.git` `.jo-history` など)は辿らない
//! - 記号の連結は追わない(輪になって戻れなくなる)

use std::path::{Path, PathBuf};

/// 当たり1つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 1 から数えた行
    pub line: u32,
    /// その行(前後は切らない。長い行は呼ぶ側が縮める)
    pub text: String,
    /// 本文の頭からのバイト位置(開いた後にそこへ飛ぶために使う)
    pub at: usize,
}

/// 1つのファイルの当たり。
#[derive(Debug, Clone)]
pub struct FileHits {
    pub path: PathBuf,
    /// ファイルの大きさ(バイト)
    pub size: u64,
    /// 最終更新(表示は呼ぶ側)
    pub mtime: Option<std::time::SystemTime>,
    pub hits: Vec<Hit>,
}

/// 探した結果の勘定。**打ち切りを隠さない**ための数
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// 中身まで見たファイルの数
    pub looked: usize,
    /// 見たファイルの大きさの合計
    pub bytes: u64,
    /// 当たったファイルの数
    pub matched: usize,
    /// 当たりの総数
    pub hits: usize,
    /// 読めなかったファイル(壊れている・権限が無い・形式を知らない)
    pub unread: usize,
    /// 上限で打ち切ったか
    pub cut: bool,
}

/// 探し方。
pub struct Query<'a> {
    /// 探す字。空なら何もしない
    pub term: String,
    /// 名前の絞り(`*.txt` の形。空なら全部)。`;` で複数
    pub glob: String,
    /// 大文字と小文字を区別する
    pub case: bool,
    /// 見るファイルの上限
    pub max_files: usize,
    /// 当たりの上限
    pub max_hits: usize,
    /// **文書の中身を取り出す**(呼ぶ側が渡す)。`None` を返したら
    /// 「この形式は知らない」— 素の字として読み直す
    pub extract: &'a dyn Fn(&Path) -> Option<String>,
}

impl<'a> Query<'a> {
    /// 素の字だけを見る(文書の中身は取り出さない)
    pub fn plain(term: &str) -> Query<'static> {
        Query {
            term: term.to_string(),
            glob: String::new(),
            case: false,
            max_files: 2000,
            max_hits: 2000,
            extract: &|_| None,
        }
    }
}

/// 名前が絞りに合うか。`*.txt;*.md` の形(`*` は「何でも」)
pub fn matches_glob(name: &str, glob: &str) -> bool {
    let g = glob.trim();
    if g.is_empty() {
        return true;
    }
    g.split(';').map(str::trim).filter(|p| !p.is_empty()).any(|p| one_glob(name, p))
}

fn one_glob(name: &str, pat: &str) -> bool {
    // 使うのは `*` だけ(`?` は要らない — 写真の道具も拡張子の絞りしか使わない)
    let name = name.to_lowercase();
    let pat = pat.to_lowercase();
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return name == pat;
    }
    let mut at = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match name[at..].find(part) {
            None => return false,
            Some(k) => {
                // 頭の断片は頭に、尻の断片は尻に付いていること
                if i == 0 && k != 0 {
                    return false;
                }
                at += k + part.len();
            }
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() && !name.ends_with(last) {
            return false;
        }
    }
    true
}

/// **この形式は素の字として読めるか。** 実行ファイルや画像を読み込んで
/// 化けた字を並べない(SFIND も名前で絞っていた)
fn plain_ext(p: &Path) -> bool {
    const OK: &[&str] = &[
        "txt", "md", "adoc", "asciidoc", "py", "toml", "json", "csv", "tsv", "log", "rs",
        "html", "htm", "xml", "yml", "yaml", "ini", "cfg", "sql", "js", "ts", "c", "h",
    ];
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OK.iter().any(|k| e.eq_ignore_ascii_case(k)))
}

/// 辿らないフォルダ(控えや版管理の中を探しても仕方がない)
fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// 探す。返りは(ファイルごとの当たり, 勘定)。
///
/// **辿る順は名前の順**(同じ場所を2回探せば同じ並びになる — 読む人が
/// 迷わない)。
/// **探した結果の報せ**(writer と calc で同じ文)。
///
/// *打ち切りも読めなかった数も言います* — 全部見たように見せません。
/// 前は両方のアプリに同じ 15 行が写してありました(2026-08-20 に1本に)。
pub fn tally_message(t: &Tally) -> String {
    let mut s = lang::i18n::trf(
        "files_hits_looked_files",
        &[&t.matched, &t.hits, &t.looked, &human_size(t.bytes)],
    );
    if t.unread > 0 {
        s.push_str(&lang::i18n::trf("not_read_2", &[&t.unread]));
    }
    if t.cut {
        s.push_str(lang::i18n::tr("stopped_early_too_many"));
    }
    s
}

pub fn walk(root: &Path, q: &Query) -> (Vec<FileHits>, Tally) {
    let mut out: Vec<FileHits> = Vec::new();
    let mut t = Tally::default();
    if q.term.is_empty() {
        return (out, t);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            t.unread += 1;
            continue;
        };
        let mut files: Vec<PathBuf> = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            // 記号の連結は追わない(輪になって戻れなくなる)
            let Ok(md) = std::fs::symlink_metadata(&p) else {
                t.unread += 1;
                continue;
            };
            if md.file_type().is_symlink() {
                continue;
            }
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if md.is_dir() {
                if !skip_dir(&name) {
                    dirs.push(p);
                }
            } else {
                files.push(p);
            }
        }
        files.sort();
        dirs.sort();
        dirs.reverse(); // pop で名前の順に出る
        stack.extend(dirs);
        for p in files {
            if t.looked >= q.max_files || t.hits >= q.max_hits {
                t.cut = true;
                return (out, t);
            }
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if !matches_glob(&name, &q.glob) {
                continue;
            }
            // **文書の中身が先。** 呼ぶ側が知らなければ素の字として読む
            let body = match (q.extract)(&p) {
                Some(s) => Some(s),
                None if plain_ext(&p) => std::fs::read_to_string(&p).ok(),
                None => continue, // 形式を知らない(画像・実行ファイル…)
            };
            let Some(body) = body else {
                t.unread += 1;
                continue;
            };
            let md = std::fs::metadata(&p).ok();
            t.looked += 1;
            t.bytes += md.as_ref().map(|m| m.len()).unwrap_or(0);
            let hits = find_in(&body, &q.term, q.case, q.max_hits - t.hits);
            if hits.is_empty() {
                continue;
            }
            t.matched += 1;
            t.hits += hits.len();
            out.push(FileHits {
                path: p,
                size: md.as_ref().map(|m| m.len()).unwrap_or(0),
                mtime: md.and_then(|m| m.modified().ok()),
                hits,
            });
        }
    }
    (out, t)
}

/// 大きさを人の読む形に(`965.81KB` — 写真の道具と同じ言い方)
pub fn human_size(n: u64) -> String {
    let k = n as f64 / 1024.0;
    if k < 1.0 {
        format!("{n}B")
    } else if k < 1024.0 {
        format!("{k:.2}KB")
    } else {
        format!("{:.2}MB", k / 1024.0)
    }
}

/// 1つの本文の中の当たり。**行ごとに1つまで**(同じ行に2回出ても
/// 一覧は1行 — 写真の道具もそうしている)
pub fn find_in(body: &str, term: &str, case: bool, cap: usize) -> Vec<Hit> {
    let mut out = Vec::new();
    if term.is_empty() || cap == 0 {
        return out;
    }
    let needle = if case { term.to_string() } else { term.to_lowercase() };
    let mut at = 0usize;
    for (i, line) in body.split('\n').enumerate() {
        let hay = if case { line.to_string() } else { line.to_lowercase() };
        if let Some(k) = hay.find(&needle) {
            // 小文字化で長さが変わる字がある(トルコ語の İ など)。
            // **位置は元の行で取り直す** — ずれた位置で開くと別の所へ飛ぶ
            let k = if case { k } else { line.to_lowercase().find(&needle).map(|_| k).unwrap_or(0) };
            out.push(Hit {
                line: i as u32 + 1,
                text: line.to_string(),
                at: at + k.min(line.len()),
            });
            if out.len() >= cap {
                break;
            }
        }
        at += line.len() + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 見本の場所。**試験ごとに別の名前**にする — 同じ名前だと、並んで走る
    /// 試験どうしが片づけ合って「無い」と言い出す(2026-08-17 に踏んだ)
    fn 見本の場所(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("owsearch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("下")).unwrap();
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::write(d.join("一.txt"), "あ\nunstructured covariance\nい\n").unwrap();
        std::fs::write(d.join("二.txt"), "何もない\n").unwrap();
        std::fs::write(d.join("下/三.md"), "# 題\nUnstructured の話\n").unwrap();
        std::fs::write(d.join(".git/四.txt"), "unstructured\n").unwrap();
        std::fs::write(d.join("絵.png"), [0u8, 1, 2]).unwrap();
        d
    }

    #[test]
    fn 下の階層まで探し_隠しフォルダは辿らない() {
        let d = 見本の場所("walk");
        let (v, t) = walk(&d, &Query::plain("unstructured"));
        let names: Vec<String> =
            v.iter().map(|f| f.path.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["一.txt", "三.md"], "辿り方が違う: {names:?}");
        assert_eq!(t.matched, 2);
        assert_eq!(t.hits, 2);
        assert!(!t.cut);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 大文字と小文字を区別できる() {
        let d = 見本の場所("case");
        let mut q = Query::plain("Unstructured");
        q.case = true;
        let (v, _) = walk(&d, &q);
        assert_eq!(v.len(), 1, "区別していない");
        assert!(v[0].path.to_string_lossy().ends_with("三.md"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 名前で絞れる() {
        let d = 見本の場所("glob");
        let mut q = Query::plain("unstructured");
        q.glob = "*.md".into();
        let (v, _) = walk(&d, &q);
        assert_eq!(v.len(), 1);
        assert!(v[0].path.to_string_lossy().ends_with("三.md"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 行と位置は開いて飛べる値になる() {
        let hits = find_in("あ\nunstructured covariance\nい\n", "covariance", false, 9);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        // 「あ\n」は 4 バイト、行頭から "unstructured " が 13 バイト
        assert_eq!(hits[0].at, 4 + 13);
        assert_eq!(hits[0].text, "unstructured covariance");
    }

    #[test]
    fn 打ち切ったらそう言う() {
        let d = 見本の場所("cut");
        let mut q = Query::plain("unstructured");
        q.max_files = 1;
        let (_, t) = walk(&d, &q);
        assert!(t.cut, "打ち切りを隠した");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 知らない形式は呼ぶ側が中身を渡せる() {
        let d = 見本の場所("ext");
        std::fs::write(d.join("五.docx"), [0u8; 8]).unwrap();
        let f = |p: &Path| -> Option<String> {
            p.extension()
                .filter(|e| *e == "docx")
                .map(|_| "unstructured を含む本文".to_string())
        };
        let q = Query {
            term: "unstructured".into(),
            glob: String::new(),
            case: false,
            max_files: 100,
            max_hits: 100,
            extract: &f,
        };
        let (v, _) = walk(&d, &q);
        assert!(
            v.iter().any(|x| x.path.to_string_lossy().ends_with("五.docx")),
            "呼ぶ側の中身が使われていない"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 大きさの言い方() {
        assert_eq!(human_size(512), "512B");
        assert_eq!(human_size(988_988), "965.81KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.00MB");
    }

    #[test]
    fn 絞りの形() {
        assert!(matches_glob("a.TXT", "*.txt"));
        assert!(matches_glob("a.md", "*.txt;*.md"));
        assert!(!matches_glob("a.png", "*.txt;*.md"));
        assert!(matches_glob("何でも", ""));
    }
}
