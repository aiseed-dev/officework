//! **プロジェクトパネルの木の状態。**
//!
//! IDE のプロジェクトパネルと同じ形(2026-08-31 発注者「IDE にあるものと
//! 同じでいい」)。ここは状態だけを持ち、絵は描かない — 描きと入力は
//! ui の側が writer と calc で共用する。
//!
//! 決め:
//!
//! - **下の階層は開いたときに読む。** 展開していないフォルダの中は
//!   ファイルシステムに触らない(深い木でも開くのが遅くならない)
//! - **状態は展開の集合と選択だけ。** 中身の一覧は毎回 [`folder::list`] で
//!   読み直す — パネルの外での変化(端末で足したファイル)も、次に開いた
//!   ときに正しく出る

use crate::folder::{self, Entry, Kind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 画面に出す1行。字下げの深さつき。
#[derive(Debug, Clone)]
pub struct Row {
    pub entry: Entry,
    /// 0 = いちばん上の階層
    pub depth: usize,
    /// フォルダで、いま展開されているか
    pub expanded: bool,
}

/// 木の状態。開いているフォルダ(根)と、展開の集合と、選択。
#[derive(Debug, Clone, Default)]
pub struct Tree {
    root: PathBuf,
    expanded: BTreeSet<PathBuf>,
    /// 選ばれている行の path。無ければ None
    pub selected: Option<PathBuf>,
}

impl Tree {
    pub fn new(root: impl Into<PathBuf>) -> Tree {
        Tree { root: root.into(), ..Default::default() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 根を差し替える(別のフォルダを開いた)。展開と選択は根ごと捨てる
    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        self.root = root.into();
        self.expanded.clear();
        self.selected = None;
    }

    pub fn is_expanded(&self, dir: &Path) -> bool {
        self.expanded.contains(dir)
    }

    /// フォルダを開く/閉じる。閉じたら、その下で選ばれていた物は
    /// フォルダ自身に選び直す(見えない行が選ばれたままにならない)
    pub fn toggle(&mut self, dir: &Path) {
        if !self.expanded.remove(dir) {
            self.expanded.insert(dir.to_path_buf());
            return;
        }
        if let Some(sel) = &self.selected {
            if sel.starts_with(dir) && sel != dir {
                self.selected = Some(dir.to_path_buf());
            }
        }
    }

    pub fn expand(&mut self, dir: &Path) {
        self.expanded.insert(dir.to_path_buf());
    }

    pub fn collapse(&mut self, dir: &Path) {
        if self.expanded.contains(dir) {
            self.toggle(dir);
        }
    }

    /// あるファイルまでの道を全部開く(開いているタブを木の中で見せる)
    pub fn reveal(&mut self, path: &Path) {
        let mut p = path.parent();
        while let Some(dir) = p {
            if !dir.starts_with(&self.root) || dir == self.root {
                break;
            }
            self.expanded.insert(dir.to_path_buf());
            p = dir.parent();
        }
        self.selected = Some(path.to_path_buf());
    }

    /// いま画面に出る行を、上から順に。展開されたフォルダだけ中を読む
    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        self.walk(&self.root, 0, &mut out);
        out
    }

    fn walk(&self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        for e in folder::list(dir) {
            let expanded = e.kind == Kind::Folder && self.is_expanded(&e.path);
            let path = e.path.clone();
            let is_dir = e.kind == Kind::Folder;
            out.push(Row { entry: e, depth, expanded });
            if is_dir && expanded {
                self.walk(&path, depth + 1, out);
            }
        }
    }

    /// ↑↓の移動。いま見えている行の中で1つ動かす
    pub fn select_step(&mut self, down: bool) {
        let rows = self.rows();
        if rows.is_empty() {
            return;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|s| rows.iter().position(|r| &r.entry.path == s));
        let next = match (cur, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(rows.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        self.selected = Some(rows[next].entry.path.clone());
    }

    /// →: フォルダなら開く。←: 開いたフォルダなら閉じ、そうでなければ
    /// 親のフォルダへ選択を上げる(Zed・VS Code と同じ動き)
    pub fn select_side(&mut self, right: bool) {
        let Some(sel) = self.selected.clone() else { return };
        let is_dir = sel.is_dir();
        if right {
            if is_dir {
                self.expand(&sel);
            }
            return;
        }
        if is_dir && self.is_expanded(&sel) {
            self.collapse(&sel);
        } else if let Some(parent) = sel.parent() {
            if parent != self.root && parent.starts_with(&self.root) {
                self.selected = Some(parent.to_path_buf());
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 試験用の庭。落ちても残らないよう Drop で消す
    struct Niwa(PathBuf);
    impl Drop for Niwa {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn niwa(name: &str) -> (Niwa, Tree) {
        let r = std::env::temp_dir().join(format!("jo-tree-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("帳票")).unwrap();
        std::fs::write(r.join("帳票/見積書.xlsx"), b"x").unwrap();
        std::fs::write(r.join("報告書.docx"), b"x").unwrap();
        let t = Tree::new(&r);
        (Niwa(r), t)
    }

    #[test]
    fn closed_folders_hide_their_children() {
        let (_d, t) = niwa("closed");
        let rows = t.rows();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0].entry.name, "帳票");
        assert_eq!(rows[0].depth, 0);
    }

    #[test]
    fn expanding_shows_children_with_depth() {
        let (_d, mut t) = niwa("expand");
        let dir = t.rows()[0].entry.path.clone();
        t.toggle(&dir);
        let rows = t.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].entry.name, "見積書.xlsx");
        assert_eq!(rows[1].depth, 1);
        t.toggle(&dir);
        assert_eq!(t.rows().len(), 2);
    }

    #[test]
    fn collapsing_moves_a_hidden_selection_to_the_folder() {
        let (_d, mut t) = niwa("collapse");
        let dir = t.rows()[0].entry.path.clone();
        t.toggle(&dir);
        let file = t.rows()[1].entry.path.clone();
        t.selected = Some(file);
        t.toggle(&dir);
        assert_eq!(t.selected.as_deref(), Some(dir.as_path()));
    }

    #[test]
    fn reveal_opens_the_way_to_a_file() {
        let (_d, mut t) = niwa("reveal");
        let file = {
            let dir = t.rows()[0].entry.path.clone();
            dir.join("見積書.xlsx")
        };
        t.reveal(&file);
        let rows = t.rows();
        assert_eq!(rows.len(), 3, "道が開いていない");
        assert_eq!(t.selected.as_deref(), Some(file.as_path()));
    }

    #[test]
    fn arrow_keys_walk_the_visible_rows() {
        let (_d, mut t) = niwa("arrows");
        t.select_step(true);
        assert_eq!(t.rows()[0].entry.path, t.selected.clone().unwrap());
        t.select_side(true); // → でフォルダが開く
        t.select_step(true);
        assert_eq!(t.selected.clone().unwrap(), t.rows()[1].entry.path);
        t.select_side(false); // ← で親へ
        assert_eq!(t.selected.clone().unwrap(), t.rows()[0].entry.path);
        t.select_side(false); // もう一度 ← で閉じる
        assert_eq!(t.rows().len(), 2);
    }
}
