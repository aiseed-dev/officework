//! plugins の .py を編集する面 — **writer と calc が共有する**
//! (2026-08-09 発注者「zed と ONLYOFFICE の合体」「.py ファイルなので
//! 両方から使えます」)。
//!
//! plugins の置き場は両方で同じ `~/.config/officework/plugins` なので、
//! 編集する面も1つでいい。ここに置くのは**中身と見せ方**だけで、
//! 「どこから開くか・保存したら何をするか」はアプリ側が決める
//! (calc なら保存でセルの関数が計算し直る)。
//!
//! **編集の芯は `kumihan::Editor`**(セル入力・数式バー・writer の本文と
//! 同じ物) — カーソル・選択・undo・IME はそこにある。ここが足すのは
//! **論理行の勘定と見せ方**だけ(行番号・行の上下・Home/End・字下げ・色分け)。
//!
//! zed の `editor` クレートは借りない。あれは 16 万行あって project / lsp /
//! workspace / multi_buffer を引き連れてくる — zed をほぼ丸ごと持ち込むことに
//! なる。借りているのは **GPUI**(zed の描画基盤)で、そこは既に土台。

// `panel` が中で `gpui::prelude::*` を引くので、飾りの trait はここに要らない
use gpui::{div, px, rgb, SharedString};
use kumihan::Editor;
use std::path::PathBuf;

/// プラグイン(.py)の置き場。**writer と calc で同じ**。
pub fn plugins_dir() -> PathBuf {
    // 正は pyrun(calc・writer と3枚で共有)。ここは呼び出し側を変えないための包み
    pyrun::plugins_dir()
}


/// 編集中の .py。
pub struct PyEdit {
    /// モジュール名(拡張子なし)
    pub name: String,
    /// **どの置き場の .py か**(2026-08-16 に UDF とマクロを分けた)。
    /// funcs = 式から呼ぶ関数 / plugins = 人が押すマクロ。
    /// 開いた所へ書き戻す — 直したら別の置き場に増えた、を起こさない
    pub dir: std::path::PathBuf,
    pub ed: Editor,
    /// 一番上に見えている行(0 起点)
    pub top: usize,
    /// 最後に保存した中身。これと違えば「書きかけ」
    pub saved: String,
}

/// 画面に出す行数(パネルの高さに合わせた固定。窓の高さは見ていない)
pub const VIEW_LINES: usize = 22;

impl PyEdit {
    /// 本文を行に割る。**空の末尾行も1行と数える**(打てる場所だから)。
    pub fn lines(&self) -> Vec<&str> {
        self.ed.text().split('\n').collect()
    }

    /// キャレットのある行と、その行頭からの桁(バイト)。
    pub fn caret(&self) -> (usize, usize) {
        let cur = self.ed.cursor();
        let head = self.ed.text()[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let row = self.ed.text()[..cur].matches('\n').count();
        (row, cur - head)
    }

    /// 行の先頭のバイト位置。
    fn line_start(&self, row: usize) -> usize {
        let t = self.ed.text();
        let mut at = 0;
        for _ in 0..row {
            match t[at..].find('\n') {
                Some(i) => at += i + 1,
                None => return t.len(),
            }
        }
        at
    }

    /// 行の長さ(バイト。改行を含まない)。
    fn line_len(&self, row: usize) -> usize {
        let st = self.line_start(row);
        self.ed.text()[st..].find('\n').unwrap_or(self.ed.text().len() - st)
    }

    /// 上下の行へ。**桁はできるだけ保つ**(行が短ければ行末)。
    pub fn move_line(&mut self, down: bool, extend: bool) {
        let (row, col) = self.caret();
        let n = self.lines().len();
        let to = if down {
            if row + 1 >= n {
                return;
            }
            row + 1
        } else {
            if row == 0 {
                return;
            }
            row - 1
        };
        let st = self.line_start(to);
        let len = self.line_len(to);
        // 桁はバイトなので、文字の途中に落ちないよう境界まで下げる
        let mut c = col.min(len);
        while c > 0 && !self.ed.text().is_char_boundary(st + c) {
            c -= 1;
        }
        self.ed.move_to(st + c, extend);
        self.follow();
    }

    pub fn home(&mut self, extend: bool) {
        let (row, _) = self.caret();
        let st = self.line_start(row);
        // 1回目は字の始まり、もう1回で行頭(Zed / VS Code と同じ作法)
        let line = &self.ed.text()[st..st + self.line_len(row)];
        let indent = line.len() - line.trim_start().len();
        let cur = self.ed.cursor();
        let to = if cur == st + indent { st } else { st + indent };
        self.ed.move_to(to, extend);
    }

    pub fn end(&mut self, extend: bool) {
        let (row, _) = self.caret();
        self.ed.move_to(self.line_start(row) + self.line_len(row), extend);
    }

    /// 改行を入れる。**前の行の字下げを引き継ぐ**(`:` で終わっていれば4つ足す)。
    pub fn newline(&mut self) {
        let (row, _) = self.caret();
        let st = self.line_start(row);
        let line = self.ed.text()[st..st + self.line_len(row)].to_string();
        let indent: String = line.chars().take_while(|c| *c == ' ').collect();
        let deeper = line.trim_end().ends_with(':');
        let mut ins = String::from("\n");
        ins.push_str(&indent);
        if deeper {
            ins.push_str("    ");
        }
        self.ed.insert(&ins);
        self.follow();
    }

    /// キャレットが見えるように送る。
    pub fn follow(&mut self) {
        let (row, _) = self.caret();
        if row < self.top {
            self.top = row;
        } else if row >= self.top + VIEW_LINES {
            self.top = row + 1 - VIEW_LINES;
        }
    }

    pub fn dirty(&self) -> bool {
        self.ed.text() != self.saved
    }
}

/// 新しい .py の下書き。**関数を1つ置いておく** — 空の画面より、
/// 直せる例がある方が始めやすい。
pub fn skeleton(name: &str) -> String {
    format!(
        "# {name}.py — plugins に置く Python\n\
         # ここに書いた def は、そのままセルから呼べる(=倍(A1) のように)。\n\
         # 保存すると、その場でシートが計算し直ります。\n\
         \n\
         def 倍(x):\n\
         \x20   return x * 2\n"
    )
}

// ---------- 色分け ----------

/// 一続きの文字と、その種類。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tok {
    Plain,
    Keyword,
    Str,
    Comment,
    Num,
    /// def / class の直後の名前(**セルから呼べる名前**なので目立たせる)
    DefName,
}

const KEYWORDS: &[&str] = &[
    "def", "class", "return", "if", "elif", "else", "for", "while", "in", "not",
    "and", "or", "import", "from", "as", "with", "try", "except", "finally",
    "raise", "lambda", "yield", "pass", "break", "continue", "global", "None",
    "True", "False", "is", "del", "assert", "async", "await",
];

/// 1行を色分けする。**行をまたぐ文字列("""…""")は追わない** — 見せ方の
/// 割り切り(間違って色が付いても中身は壊れない)。
pub fn colorize(line: &str) -> Vec<(String, Tok)> {
    let b: Vec<char> = line.chars().collect();
    let mut out: Vec<(String, Tok)> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    let mut after_def = false;
    let push = |out: &mut Vec<(String, Tok)>, plain: &mut String| {
        if !plain.is_empty() {
            out.push((std::mem::take(plain), Tok::Plain));
        }
    };
    while i < b.len() {
        let c = b[i];
        if c == '#' {
            push(&mut out, &mut plain);
            out.push((b[i..].iter().collect(), Tok::Comment));
            return out;
        }
        if c == '"' || c == '\'' {
            push(&mut out, &mut plain);
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += 1;
            }
            let end = (j + 1).min(b.len());
            out.push((b[i..end].iter().collect(), Tok::Str));
            i = end;
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            let st = i;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            let w: String = b[st..i].iter().collect();
            push(&mut out, &mut plain);
            let kind = if after_def {
                after_def = false;
                Tok::DefName
            } else if KEYWORDS.contains(&w.as_str()) {
                if w == "def" || w == "class" {
                    after_def = true;
                }
                Tok::Keyword
            } else if w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                Tok::Num
            } else {
                Tok::Plain
            };
            out.push((w, kind));
            continue;
        }
        plain.push(c);
        i += 1;
    }
    push(&mut out, &mut plain);
    out
}

pub fn tok_color(t: Tok) -> gpui::Rgba {
    match t {
        Tok::Keyword => rgb(0x0F62A8),
        Tok::Str => rgb(0xA3324A),
        Tok::Comment => rgb(0x7A8590),
        Tok::Num => rgb(0x1B6E3C),
        Tok::DefName => rgb(0x6A3AB2),
        Tok::Plain => rgb(0x1B1B1B),
    }
}

/// .py の編集面を描く。**表の上に大きく重ねる**(パネルの作法は
/// 他の小窓と同じ — 外側の受け皿は聞き手を持たない)。
pub fn panel(
    p: &PyEdit,
    us: f32,
    font: SharedString,
    ask: bool,
) -> gpui::AnyElement {
    use gpui::prelude::*;
    let lines = p.lines();
    let (crow, ccol) = p.caret();
    let last = (p.top + VIEW_LINES).min(lines.len());
    let mut body = div().flex().flex_col();
    for (i, line) in lines[p.top.min(lines.len())..last].iter().enumerate() {
        let row = p.top + i;
        let mut ln = div().flex().flex_row().items_start();
        // 行番号(いまの行は濃く)
        ln = ln.child(
            div()
                .w(px(us * 34.0))
                .flex_none()
                .pr_2()
                .text_color(if row == crow { rgb(0x1B6E3C) } else { rgb(0xAAB2BA) })
                .child(SharedString::from(format!("{:>3}", row + 1))),
        );
        // 中身。いまの行だけキャレットを差し込む(| で見せる — 数式バーと同じ割り切り)
        let mut text = (*line).to_string();
        if row == crow {
            let at = ccol.min(text.len());
            text.insert(at, '|');
        }
        let mut code = div().flex().flex_row().flex_wrap();
        for (frag, tok) in colorize(&text) {
            code = code.child(
                div().flex_none().text_color(tok_color(tok)).child(SharedString::from(frag)),
            );
        }
        ln = ln.child(code);
        if row == crow {
            ln = ln.bg(rgb(0xF2F7F4));
        }
        body = body.child(ln);
    }
    let title = format!("{}.py{}", p.name, if p.dirty() { " *" } else { "" });
    let foot = if ask {
        crate::t!("unsaved_changes_ctrl_s").to_string()
    } else {
        crate::t!("ctrl_s_save_cell")
            .to_string()
    };
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(us * 620.0))
                .p_3()
                .rounded_md()
                .bg(rgb(0xFBFCFD))
                .border_1()
                .border_color(rgb(0x1B6E3C))
                .shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(px(us * 12.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x1B6E3C))
                        .child(SharedString::from(title))
                        .child(SharedString::from(format!("{}:{}", crow + 1, ccol + 1))),
                )
                .child(
                    div()
                        .mt_1p5()
                        .px_2()
                        .py_1()
                        .bg(rgb(0xFFFFFF))
                        .border_1()
                        .border_color(rgb(0xC6CDD3))
                        .rounded_sm()
                        .font_family(font)
                        .text_size(px(us * 12.5))
                        .child(body),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(us * 10.5))
                        .text_color(if ask { rgb(0xB3261E) } else { rgb(0x66707A) })
                        .child(SharedString::from(foot)),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> PyEdit {
        PyEdit {
            name: "試".into(),
            dir: std::path::PathBuf::from("/tmp"),
            ed: Editor::new(text),
            top: 0,
            saved: text.into(),
        }
    }

    #[test]
    fn counts_lines_and_columns() {
        let mut e = p("abc\ndef\n");
        e.ed.move_to(5, false); // 2行目の 1 桁目
        assert_eq!(e.caret(), (1, 1));
        // 末尾の空行も1行と数える(打てる場所だから)
        assert_eq!(e.lines().len(), 3);
    }

    #[test]
    fn moving_up_and_down_keeps_the_column() {
        let mut e = p("abcdef\nxy\nghijkl");
        e.ed.move_to(5, false); // 1行目の 5 桁目
        e.move_line(true, false);
        assert_eq!(e.caret(), (1, 2), "短い行では行末に落ちる");
        e.move_line(true, false);
        assert_eq!(e.caret().0, 2);
        // 一番上より上・一番下より下へは行かない
        e.ed.move_to(0, false);
        e.move_line(false, false);
        assert_eq!(e.caret(), (0, 0));
    }

    #[test]
    fn columns_survive_japanese_lines() {
        // バイトで持っているので、文字の途中に落ちると即座に化ける
        let mut e = p("あいうえお\nか");
        e.end(false);
        e.move_line(true, false);
        let (r, c) = e.caret();
        assert_eq!(r, 1);
        assert!(e.ed.text().is_char_boundary(e.ed.cursor()), "文字の途中に落ちた: 桁 {c}");
    }

    #[test]
    fn newline_carries_the_indent() {
        let mut e = p("def f(x):");
        e.end(false);
        e.newline();
        assert_eq!(e.ed.text(), "def f(x):\n    ", ": の後は4つ深くする");
        e.ed.insert("return x");
        e.newline();
        assert_eq!(e.ed.text(), "def f(x):\n    return x\n    ", "字下げを引き継ぐ");
    }

    #[test]
    fn home_toggles_between_first_text_and_line_start() {
        let mut e = p("    return x");
        e.end(false);
        e.home(false);
        assert_eq!(e.ed.cursor(), 4, "1回目は字の始まり");
        e.home(false);
        assert_eq!(e.ed.cursor(), 0, "2回目は行頭");
    }

    #[test]
    fn syntax_coloring() {
        let v = colorize("def 倍(x):  # 二倍");
        assert_eq!(v[0], ("def".into(), Tok::Keyword));
        assert_eq!(v[1].1, Tok::Plain); // 空白
        assert_eq!(v[2], ("倍".into(), Tok::DefName), "セルから呼べる名前を目立たせる");
        assert!(v.iter().any(|(s, t)| *t == Tok::Comment && s.contains("二倍")));
        // 文字列は閉じていなくても行末まで
        let v = colorize("s = \"開いたまま");
        assert!(v.iter().any(|(_, t)| *t == Tok::Str));
    }

    #[test]
    fn an_unfinished_line_is_detected() {
        let mut e = p("a");
        assert!(!e.dirty());
        e.ed.insert("b");
        assert!(e.dirty(), "書きかけを見落とすと黙って捨てることになる");
    }
}
