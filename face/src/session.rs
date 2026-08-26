//! **前回の姿**(SEKKEI「前回の姿を戻す」。2026-08-19 発注者「これは当然でしょう」)。
//!
//! 開き直したときに、前に開いていたフォルダ・ファイル・見ていたタブを戻します。
//!
//! *`settings.toml` に混ぜません。* あちらは人が書く設定で、こちらは機械が
//! 書き換える記録です。混ぜると、人が書いた行の隣を機械が勝手に足したり
//! 消したりすることになります。
//!
//! 綴りは素直な行の並びです。読む道具を増やさないのがこの層の作法です。
//!
//! ```text
//! folder /home/dev/帳簿
//! * /home/dev/帳簿/売上台帳.sheet.adoc
//! - /home/dev/帳簿/報告書.adoc
//! ```
//!
//! 頭の1字が印です。`folder` は開いていたフォルダ、`*` は見ていたタブ、
//! `-` はそれ以外のタブ。**印を先に置く**ので、道に空白が入っていても切れます。

use std::path::{Path, PathBuf};

/// 前回の姿。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    /// 開いていたフォルダ
    pub folder: Option<PathBuf>,
    /// 開いていたファイル(タブの並び)
    pub files: Vec<PathBuf>,
    /// 見ていたタブは何枚目か
    pub at: usize,
}

/// 置き場(`settings.toml` の隣)。
pub fn path() -> PathBuf {
    lang::config_dir().join("session.txt")
}

/// 綴る。
pub fn to_text(s: &Session) -> String {
    let mut out = String::new();
    if let Some(d) = &s.folder {
        out.push_str(&format!("folder {}\n", d.display()));
    }
    for (i, f) in s.files.iter().enumerate() {
        out.push_str(&format!("{} {}\n", if i == s.at { "*" } else { "-" }, f.display()));
    }
    out
}

/// 読む。**知らない行は黙って飛ばします** — 前の版が書いた物や、人が
/// 手で触った物で起動が止まらないように。
pub fn from_text(t: &str) -> Session {
    let mut s = Session::default();
    for line in t.lines() {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("folder ") {
            s.folder = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("* ") {
            s.at = s.files.len();
            s.files.push(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("- ") {
            s.files.push(PathBuf::from(rest));
        }
    }
    s
}

/// 控える。**タブを開く・閉じる・切り替えるたびに呼びます** —
/// 終了のときだけ書くと、落ちたときに前回の姿が残りません。
pub fn save(s: &Session) {
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, to_text(s));
}

/// 読み出す。無ければ空。
pub fn load() -> Session {
    std::fs::read_to_string(path()).map(|t| from_text(&t)).unwrap_or_default()
}

/// **いま在るファイルだけに絞る。** 消えた物・動いた物は開けません。
///
/// 返すのは (絞った姿, 見つからなかった数)。呼ぶ側は数を画面で言うこと —
/// *黙って減らさない*。「前に3枚開いていたはずが2枚」を、使う人が
/// 自分の記憶違いだと思ってしまいます。
pub fn prune(s: &Session) -> (Session, usize) {
    let mut out = Session { folder: s.folder.clone().filter(|d| d.is_dir()), ..Default::default() };
    let 見ていた = s.files.get(s.at).cloned();
    let mut dropped = 0usize;
    for f in &s.files {
        if f.is_file() {
            out.files.push(f.clone());
        } else {
            dropped += 1;
        }
    }
    // 見ていたタブが残っていればそこへ、消えていれば先頭へ
    out.at = 見ていた
        .and_then(|w| out.files.iter().position(|f| *f == w))
        .unwrap_or(0);
    (out, dropped)
}

/// 前の版から上げた人のための**1回だけの移し替え**。
///
/// 「前回のフォルダ」は `settings.toml` の `folder` に入っていました。
/// `session.txt` がまだ無いときだけ、そこから拾います —
/// **前の版で使っていた場所を無かったことにしない**ためです。
/// 2回目からは `session.txt` が在るので、ここは通りません。
pub fn 引き継ぐ(古いフォルダ: Option<String>) -> Session {
    if path().exists() {
        return load();
    }
    Session {
        folder: 古いフォルダ.map(PathBuf::from).filter(|d| d.is_dir()),
        ..Default::default()
    }
}

/// いま開いている物から姿を作る(`officework` が呼ぶ)。
pub fn of(folder: Option<&Path>, files: &[PathBuf], at: usize) -> Session {
    Session { folder: folder.map(|d| d.to_path_buf()), files: files.to_vec(), at }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn snapshot() -> Session {
        Session {
            folder: Some(PathBuf::from("/tmp/帳簿")),
            files: vec![
                PathBuf::from("/tmp/帳簿/報告書.adoc"),
                PathBuf::from("/tmp/帳簿/売上台帳.sheet.adoc"),
            ],
            at: 1,
        }
    }

    #[test]
    fn 綴って読むと同じ姿になる() {
        assert_eq!(from_text(&to_text(&snapshot())), snapshot());
    }

    #[test]
    fn 見ていたタブに印が付く() {
        let t = to_text(&snapshot());
        assert!(t.contains("* /tmp/帳簿/売上台帳.sheet.adoc"), "{t}");
        assert!(t.contains("- /tmp/帳簿/報告書.adoc"), "{t}");
    }

    /// **道に空白があっても切れない。** 印を先に置いているのがその理由
    #[test]
    fn 空白のある道も読める() {
        let s = from_text("folder /tmp/私 の 帳簿\n* /tmp/私 の 帳簿/売上 4月.sheet.adoc\n");
        assert_eq!(s.folder, Some(PathBuf::from("/tmp/私 の 帳簿")));
        assert_eq!(s.files, vec![PathBuf::from("/tmp/私 の 帳簿/売上 4月.sheet.adoc")]);
    }

    /// 知らない行で起動が止まらない
    #[test]
    fn 知らない行は飛ばす() {
        let s = from_text("# 覚え\nzoom 120\n- /tmp/a.adoc\n");
        assert_eq!(s.files, vec![PathBuf::from("/tmp/a.adoc")]);
    }

    #[test]
    fn 空でも読める() {
        assert_eq!(from_text(""), Session::default());
    }

    #[test]
    fn 無くなったファイルは落として数える() {
        let d = std::env::temp_dir().join("ow-session-試験");
        std::fs::create_dir_all(&d).unwrap();
        let 在る = d.join("在る.adoc");
        std::fs::write(&在る, "= 在る\n").unwrap();
        let missing = d.join("無い.adoc");
        let _ = std::fs::remove_file(&missing);
        let s = Session { folder: Some(d.clone()), files: vec![missing.clone(), 在る.clone()], at: 0 };
        let (out, dropped) = prune(&s);
        assert_eq!(dropped, 1);
        assert_eq!(out.files, vec![在る.clone()]);
        // 見ていたのは消えた側だったので、先頭へ寄せる
        assert_eq!(out.at, 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 見ていたタブが残っていれば、**そこへ戻る**(番号ではなく道で引く)
    #[test]
    fn 見ていたタブは道で引き直す() {
        let d = std::env::temp_dir().join("ow-session-試験2");
        std::fs::create_dir_all(&d).unwrap();
        let a = d.join("a.adoc");
        let b = d.join("b.adoc");
        std::fs::write(&a, "= a\n").unwrap();
        std::fs::write(&b, "= b\n").unwrap();
        let missing = d.join("消えた.adoc");
        let s = Session { folder: Some(d.clone()), files: vec![missing, a.clone(), b.clone()], at: 2 };
        let (out, dropped) = prune(&s);
        assert_eq!(dropped, 1);
        // b を見ていた。前は3枚目だったが、いまは2枚目
        assert_eq!(out.files, vec![a, b.clone()]);
        assert_eq!(out.files[out.at], b, "見ていたタブが変わった");
        let _ = std::fs::remove_dir_all(&d);
    }
}
