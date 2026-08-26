//! **上書きの前の控え**(バージョン履歴)。writer と calc が同じ物を使います。
//!
//! 置き場は元のファイルと同じフォルダの
//! `.jo-history/<ファイル名>/<日時>.<元と同じ拡張子>`。9世代まで残します。
//!
//! 名前は**その中身を保存した日時**(ファイルの更新時刻)です。控えを作った
//! 時刻ではありません — 「いつの姿か」が知りたいのであって、「いつ控えたか」
//! ではないからです。
//!
//! ## 外部の `date` を呼ばない(2026-08-20 に直した)
//!
//! 前は writer と calc がそれぞれ `date -r <ファイル>` を起こしていました。
//! **これは GNU の書き方で、macOS と Windows では通りません** —
//! BSD の `date -r` は秒数を取るので、ファイルの道を渡すと失敗します。
//! 失敗すると日時が `0` になり、**控えが全部 `0.docx` という同じ名前になって
//! 上書きし合います**。つまり Linux 以外ではバージョン履歴が1世代しか
//! 残らず、しかも日付が出ていませんでした。
//!
//! `ui::now_stamp` には前から「外部の date を呼ばない — 呼ぶと Windows で
//! 動かない」と書いてあり、そちらは直っていました。ここだけが残っていました。

use std::path::{Path, PathBuf};

/// 残す世代の数。
const GEN: usize = 9;

/// 地方時のずれ(秒)。**1つのプロセスで1回だけ**調べます。
///
/// `std::time` は時間帯を知りません。`/etc/localtime` を読む気は無いので、
/// `TZ_OFFSET_SECS` があればそれを、無ければ `date +%z` を1回だけ聞き、
/// それも駄目なら UTC のまま出します(`ui::now_stamp` と同じ決め)。
/// **1回だけ**なので、保存のたびにプロセスを起こすことにはなりません。
fn local_time_offset() -> i64 {
    static ZURE: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *ZURE.get_or_init(|| {
        if let Some(v) = std::env::var("TZ_OFFSET_SECS").ok().and_then(|v| v.parse::<i64>().ok()) {
            return v;
        }
        std::process::Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let sign = if s.starts_with('-') { -1 } else { 1 };
                let h: i64 = s.get(1..3)?.parse().ok()?;
                let mi: i64 = s.get(3..5)?.parse().ok()?;
                Some(sign * (h * 3600 + mi * 60))
            })
            .unwrap_or(0)
    })
}

/// 1970 からの秒 → `YYYYMMDD-HHMMSS`(控えのファイル名)。
///
/// 暦の算法は `sheet::civil_from_days` の1本を使います —
/// **暦を2箇所に持たない**。
pub fn stamp(epoch_secs: i64) -> String {
    let secs = epoch_secs + local_time_offset();
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = sheet::civil_from_days(days);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// 控えの置き場(`.jo-history/<ファイル名>/`)。
fn store_dir(p: &Path) -> Option<PathBuf> {
    let name = p.file_name()?.to_string_lossy().to_string();
    Some(p.parent().unwrap_or(Path::new(".")).join(".jo-history").join(name))
}

/// **上書きの前に、いまの中身を控える。**
///
/// 控えられなくても**保存は止めません** — 控えは保険であって、
/// 保険が掛けられないことを理由に本体の保存を諦めさせるのは逆です。
pub fn keep(p: &Path) {
    let Some(dir) = store_dir(p) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let secs = std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // **控えの拡張子は元と同じ**(`.adoc` のブックを `.xlsx` と名乗らない)
    let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("bak");
    let _ = std::fs::copy(p, dir.join(format!("{}.{ext}", stamp(secs))));
    // 増えすぎたら古い控えから消す
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut old: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        old.sort();
        while old.len() > GEN {
            let _ = std::fs::remove_file(old.remove(0));
        }
    }
}

/// 控えの一覧(新しい順)。`(画面に出す名前, 道)`。
pub fn list(p: Option<&Path>) -> Vec<(String, PathBuf)> {
    let Some(dir) = p.and_then(store_dir) else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    v.reverse();
    v.into_iter()
        .map(|q| {
            let stem = q.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let kb = std::fs::metadata(&q).map(|m| m.len() / 1024).unwrap_or(0);
            (format!("{}({kb} KB)", display_name(&stem)), q)
        })
        .collect()
}

/// `20260804-183012` → `2026-08-04 18:30`。読めない名前はそのまま出します
/// (**黙って作り変えない** — 古い控えや人が置いた物かもしれません)。
fn display_name(stem: &str) -> String {
    if stem.len() >= 13 && stem.is_ascii() && stem.as_bytes()[8] == b'-' {
        format!(
            "{}-{}-{} {}:{}",
            &stem[0..4], &stem[4..6], &stem[6..8], &stem[9..11], &stem[11..13]
        )
    } else {
        stem.to_string()
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// **外部の `date` を呼ばずに日時が出る。** ここが移植性の要
    #[test]
    fn 秒から名前を作る() {
        // TZ のずれを 0 に固定して比べる(地方時は機械ごとに違う)
        unsafe { std::env::set_var("TZ_OFFSET_SECS", "0") };
        // 2026-08-04 18:30:12 UTC
        let s = stamp(1_785_868_212);
        assert_eq!(s.len(), 15, "{s}");
        assert_eq!(&s[8..9], "-", "{s}");
        assert!(s.starts_with("2026"), "{s}");
    }

    #[test]
    fn 名前を読みやすく直す() {
        assert_eq!(display_name("20260804-183012"), "2026-08-04 18:30");
    }

    /// **読めない名前はそのまま。** 古い控え(`0` になっていた物)や、
    /// 人が置いた物を勝手に作り変えない
    #[test]
    fn 読めない名前はそのまま() {
        assert_eq!(display_name("0"), "0");
        assert_eq!(display_name("控え"), "控え");
        assert_eq!(display_name("2026080418301"), "2026080418301", "区切りが無い");
    }

    #[test]
    fn 控えて一覧に出る() {
        let d = std::env::temp_dir().join("ops-history-試験");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("売上.sheet.adoc");
        std::fs::write(&f, "= 売上\n").unwrap();
        keep(&f);
        let v = list(Some(&f));
        assert_eq!(v.len(), 1, "控えが出ない: {v:?}");
        // **拡張子は元と同じ**
        assert!(v[0].1.to_string_lossy().ends_with(".adoc"), "{:?}", v[0].1);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 9世代を超えたら古い方から落ちる
    #[test]
    fn 九世代で打ち止め() {
        let d = std::env::temp_dir().join("ops-history-試験2");
        let _ = std::fs::remove_dir_all(&d);
        let hist = d.join(".jo-history").join("a.txt");
        std::fs::create_dir_all(&hist).unwrap();
        for i in 0..12 {
            std::fs::write(hist.join(format!("2026080{}-000000.txt", i % 9)), "x").unwrap();
        }
        let f = d.join("a.txt");
        std::fs::write(&f, "x").unwrap();
        keep(&f);
        let n = std::fs::read_dir(&hist).unwrap().count();
        assert!(n <= GEN, "{n} 件残っている");
        let _ = std::fs::remove_dir_all(&d);
    }
}
