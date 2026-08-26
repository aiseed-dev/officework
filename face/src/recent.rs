//! **最近使ったファイル — 1つの一覧**(統合の段8。2026-08-19 発注者
//! 「最近使った物・新規・言語などの設定は1箇所になる」)。
//!
//! 前は `recent-writer.txt` と `recent-calc.txt` に分かれていました。
//! 中身は**同じコードの写し**(ファイル名だけ違う)で、しかも使う人から見ると
//! *ファイルはファイル*です。「昨日さわった売上台帳」を探すときに、
//! 表の一覧と文書の一覧のどちらを見るかを考えさせる理由がありません。
//!
//! 置き場は `~/.config/officework/recent.txt`。新しい順に 12 件まで。

use std::path::{Path, PathBuf};

/// 覚えておく数。
const 上限: usize = 12;

/// 置き場。
pub fn path() -> PathBuf {
    設定の場().join("recent.txt")
}

/// 設定の置き場。**試験は `at` 付きの関数を直に呼ぶ**ので、ここは本番だけ。
fn 設定の場() -> PathBuf {
    lang::config_dir()
}

/// 使ったと控える。**同じ物は上に上げ直します**(二重に並べない)。
pub fn note(p: &Path) {
    note_at(&設定の場(), p)
}

/// 置き場を指して控える。**試験はこちらを呼びます** — `$HOME` を書き換える
/// 試験は、並べて走らせると他の試験を壊します(2026-08-20 に実際に壊した)。
pub fn note_at(dir: &Path, p: &Path) {
    let rf = dir.join("recent.txt");
    if let Some(dir) = rf.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut list = read_list(&rf);
    let me = p.to_string_lossy().to_string();
    list.retain(|x| *x != me);
    list.insert(0, me);
    list.truncate(上限);
    let _ = std::fs::write(&rf, list.join("\n"));
}

/// 一覧(新しい順)。**いま在る物だけ**を返します — 消えたファイルを
/// 押させても開けません。
pub fn list() -> Vec<PathBuf> {
    list_at(&設定の場())
}

/// 置き場を指して読む(試験はこちら)。
pub fn list_at(dir: &Path) -> Vec<PathBuf> {
    引き継ぐ(dir);
    read_list(&dir.join("recent.txt")).into_iter().map(PathBuf::from).filter(|p| p.exists()).collect()
}

fn read_list(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// **前の版から上げた人の控えを1回だけ拾う。**
///
/// `recent.txt` がまだ無いときだけ、writer と calc の古い控えを混ぜます。
/// *使っていた履歴を無かったことにしない*ためです。2回目からは
/// `recent.txt` が在るので、ここは通りません。
///
/// 混ぜる順は**writer が先、calc が後**の交互ではなく、そのまま繋いで
/// 重複だけ落とします。どちらが新しいかを比べる材料が無いので、
/// **順を作り話しない** — 使えば自然に上へ上がります。
fn 引き継ぐ(dir: &Path) {
    let rf = dir.join("recent.txt");
    if rf.exists() {
        return;
    }
    let mut 混ぜ: Vec<String> = Vec::new();
    for 古 in ["recent-writer.txt", "recent-calc.txt"] {
        for l in read_list(&dir.join(古)) {
            if !混ぜ.contains(&l) {
                混ぜ.push(l);
            }
        }
    }
    if 混ぜ.is_empty() {
        return;
    }
    混ぜ.truncate(上限);
    if let Some(dir) = rf.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&rf, 混ぜ.join("\n"));
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// **`$HOME` を書き換えない。** 書き換える試験は、並べて走らせると
    /// 他の試験を壊します(2026-08-20 に実際に壊した)。置き場を引数で渡す
    /// `note_at` / `list_at` を呼びます
    fn 試験の場(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("face-recent-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn 新しい順に並び二重に入らない() {
        let d = 試験の場("順");
        let a = d.join("a.adoc");
        let b = d.join("b.sheet.adoc");
        std::fs::write(&a, "= a\n").unwrap();
        std::fs::write(&b, "= b\n").unwrap();
        note_at(&d, &a);
        note_at(&d, &b);
        note_at(&d, &a); // もう一度 a → 上に上がる。二重にはならない
        let v = list_at(&d);
        assert_eq!(v, vec![a.clone(), b.clone()], "{v:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 無くなったファイルは出さない() {
        let d = 試験の場("消えた");
        let a = d.join("a.adoc");
        std::fs::write(&a, "= a\n").unwrap();
        note_at(&d, &a);
        note_at(&d, &d.join("無い.adoc"));
        assert_eq!(list_at(&d), vec![a.clone()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **前の版の2つの控えを1回だけ拾う**(使っていた履歴を捨てない)
    #[test]
    fn 古い控えを引き継ぐ() {
        let d = 試験の場("引き継ぎ");
        let w = d.join("文書.adoc");
        let c = d.join("表.sheet.adoc");
        std::fs::write(&w, "= w\n").unwrap();
        std::fs::write(&c, "= c\n").unwrap();
        std::fs::write(d.join("recent-writer.txt"), w.display().to_string()).unwrap();
        std::fs::write(d.join("recent-calc.txt"), c.display().to_string()).unwrap();
        let v = list_at(&d);
        assert!(v.contains(&w) && v.contains(&c), "両方を拾えていない: {v:?}");
        // 2回目は新しい控えだけを見る(古い方を混ぜ直さない)
        note_at(&d, &w);
        assert_eq!(list_at(&d)[0], w);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 上限で切る() {
        let d = 試験の場("上限");
        for i in 0..(上限 + 3) {
            let f = d.join(format!("{i}.adoc"));
            std::fs::write(&f, "x").unwrap();
            note_at(&d, &f);
        }
        assert_eq!(list_at(&d).len(), 上限);
        let _ = std::fs::remove_dir_all(&d);
    }
}
