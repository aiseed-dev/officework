//! 編集モデル — カーソル・選択・undo/redo・IMEの未確定文字。UI非依存。
//!
//! 位置はすべて**バイト位置**(Rustの文字列と同じ物差し)で持つ。
//! GPUIのIME APIは UTF-16 の位置で来るので、境界で変換する(utf16.rs 相当は
//! ここに小さく持つ)。日本語は UTF-8 で3バイト・UTF-16 で1〜2単位なので、
//! ここを混同すると即座に文字化けする — だから型で分けず、関数名で分ける。
//!
//! IMEの要点(K2の難所):
//!   変換中の文字列は「まだ文書ではない」。marked(未確定)として本文に載せつつ、
//!   確定(commit)するまでは undo の単位にしない。変換をやめたら丸ごと消える。

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    text: String,
    cursor: usize,
    anchor: usize,
}

/// 一つの編集可能なテキスト。
#[derive(Debug, Clone)]
pub struct Editor {
    text: String,
    /// 選択の可動端(キャレット)。バイト位置
    cursor: usize,
    /// 選択の固定端。cursor と同じなら選択なし
    anchor: usize,
    /// IMEの未確定範囲(バイト位置)。変換中だけ Some
    marked: Option<Range<usize>>,
    /// 変換が始まる前の状態。確定したときの undo はここへ戻る
    /// (未確定を消した後の状態に戻ってはいけない — 選択を置き換える変換で壊れる)
    pre_ime: Option<Snapshot>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// **直前の数学オートコレクトの控え。** 打っている途中で勝手に
    /// 置き換わるので、**直後の Backspace で綴りに戻せる**ようにこれを持つ
    /// (Word と同じ作法)。別の打鍵をしたら捨てる — ずっと覚えていると、
    /// 無関係な Backspace が古い綴りを吐き出す
    autocorrected: Option<AutoFix>,
}

/// 置き換えたばかりの記号の控え([`Editor::autocorrect_math`])
#[derive(Debug, Clone)]
struct AutoFix {
    /// 置き換えた直後のキャレットの位置。**ここに居るときだけ**戻せる
    caret: usize,
    /// 入れた記号のバイト範囲(区切りの手前)
    sym: Range<usize>,
    /// 元の綴り(`\alpha`)
    was: String,
}

/// 数学オートコレクト(2026-08-13、台帳「数学オートコレクト」)。
///
/// `\alpha` のような綴りを記号に替える。表は本家(と Word)の顔ぶれから
/// **記号1文字で済む物だけ**。分数・上付き・根号の中身のような「組み方」が
/// 要る物は入れない — 一列の文字では正しく出せず、出せない物を出せるように
/// 見せることになる(数式そのものは LaTeX で受けて Python に組ませる)。
///
/// 引き当ては**綴りぴったり**。`\alphabet` を α+bet にはしない
pub fn math_symbol(word: &str) -> Option<&'static str> {
    // (綴り, 記号)。綴りは `\` 込み
    const T: &[(&str, &str)] = &[
        // ギリシャ文字(小)
        ("\\alpha", "α"), ("\\beta", "β"), ("\\gamma", "γ"), ("\\delta", "δ"),
        ("\\epsilon", "ε"), ("\\zeta", "ζ"), ("\\eta", "η"), ("\\theta", "θ"),
        ("\\iota", "ι"), ("\\kappa", "κ"), ("\\lambda", "λ"), ("\\mu", "μ"),
        ("\\nu", "ν"), ("\\xi", "ξ"), ("\\pi", "π"), ("\\rho", "ρ"),
        ("\\sigma", "σ"), ("\\tau", "τ"), ("\\upsilon", "υ"), ("\\phi", "φ"),
        ("\\chi", "χ"), ("\\psi", "ψ"), ("\\omega", "ω"),
        // ギリシャ文字(大)
        ("\\Gamma", "Γ"), ("\\Delta", "Δ"), ("\\Theta", "Θ"), ("\\Lambda", "Λ"),
        ("\\Xi", "Ξ"), ("\\Pi", "Π"), ("\\Sigma", "Σ"), ("\\Phi", "Φ"),
        ("\\Psi", "Ψ"), ("\\Omega", "Ω"),
        // 演算と比較
        ("\\times", "×"), ("\\div", "÷"), ("\\pm", "±"), ("\\mp", "∓"),
        ("\\cdot", "·"), ("\\ne", "≠"), ("\\neq", "≠"), ("\\le", "≤"),
        ("\\leq", "≤"), ("\\ge", "≥"), ("\\geq", "≥"), ("\\approx", "≈"),
        ("\\equiv", "≡"), ("\\propto", "∝"), ("\\sim", "∼"),
        // 大きい記号(**中身を組まない**ので、記号そのものだけ)
        ("\\sum", "∑"), ("\\prod", "∏"), ("\\int", "∫"), ("\\sqrt", "√"),
        ("\\partial", "∂"), ("\\nabla", "∇"), ("\\infty", "∞"),
        // 矢印
        ("\\to", "→"), ("\\rightarrow", "→"), ("\\leftarrow", "←"),
        ("\\leftrightarrow", "↔"), ("\\Rightarrow", "⇒"), ("\\Leftarrow", "⇐"),
        ("\\Leftrightarrow", "⇔"), ("\\uparrow", "↑"), ("\\downarrow", "↓"),
        // 集合と論理
        ("\\in", "∈"), ("\\notin", "∉"), ("\\subset", "⊂"), ("\\supset", "⊃"),
        ("\\cup", "∪"), ("\\cap", "∩"), ("\\emptyset", "∅"),
        ("\\forall", "∀"), ("\\exists", "∃"), ("\\therefore", "∴"),
        ("\\because", "∵"),
        // 図形と単位
        ("\\angle", "∠"), ("\\perp", "⊥"), ("\\parallel", "∥"),
        ("\\deg", "°"), ("\\degree", "°"), ("\\circ", "∘"), ("\\bullet", "•"),
        ("\\ldots", "…"), ("\\permil", "‰"), ("\\micro", "µ"),
        ("\\yen", "¥"), ("\\euro", "€"), ("\\pound", "£"),
    ];
    T.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

/// カーソルの手前にある「オートコレクトの相手」の綴りを探す。
///
/// `\` から始まり英字だけが続く塊。`\` が無ければ相手ではない —
/// **ふつうの言葉を勝手に置き換えない**のがこの形の眼目で、
/// `pi` を打っただけで π になっては帳票が書けない
fn word_before(text: &str, at: usize) -> Option<(usize, &str)> {
    let head = &text[..at];
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphabetic() || *c == '\\')
        .last()
        .map(|(i, _)| i)?;
    let w = &head[start..];
    if !w.starts_with('\\') || w.len() < 2 {
        return None;
    }
    Some((start, w))
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new("")
    }
}

impl Editor {
    pub fn new(text: &str) -> Editor {
        let n = text.len();
        Editor {
            text: text.to_string(),
            cursor: n,
            anchor: n,
            marked: None,
            pre_ime: None,
            undo: Vec::new(),
            redo: Vec::new(),
            autocorrected: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }
    pub fn selection(&self) -> Range<usize> {
        let (a, b) = (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        a..b
    }
    pub fn has_selection(&self) -> bool {
        self.anchor != self.cursor
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { text: self.text.clone(), cursor: self.cursor, anchor: self.anchor }
    }

    /// undo の区切りを打つ。IMEの未確定中は打たない(確定して初めて1手)。
    fn checkpoint(&mut self) {
        let s = self.snapshot();
        if self.undo.last().map(|x| &x.text) != Some(&s.text) {
            self.undo.push(s);
            self.redo.clear();
        }
    }

    // ---------- 移動と選択 ----------

    /// 文字単位で移動。extend=true なら選択を伸ばす。
    pub fn move_char(&mut self, forward: bool, extend: bool) {
        let p = if forward {
            next_boundary(&self.text, self.cursor)
        } else {
            prev_boundary(&self.text, self.cursor)
        };
        self.cursor = p;
        if !extend {
            self.anchor = p;
        }
        self.marked = None;
    }

    pub fn move_to(&mut self, byte: usize, extend: bool) {
        let p = clamp_boundary(&self.text, byte);
        self.cursor = p;
        if !extend {
            self.anchor = p;
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    // ---------- 入力 ----------

    /// 選択を置き換えて文字列を入れる(通常の入力・貼り付け)。undo の1手。
    pub fn insert(&mut self, s: &str) {
        // 別の打鍵をしたらオートコレクトの控えは捨てる — 覚えたままだと、
        // 位置がたまたま戻ったときに無関係な Backspace が古い綴りを吐く
        self.autocorrected = None;
        self.checkpoint();
        let r = self.replace_range();
        self.text.replace_range(r.clone(), s);
        self.cursor = r.start + s.len();
        self.anchor = self.cursor;
        self.marked = None;
    }

    /// **数学オートコレクトを掛けて、区切りの文字を続けて入れる。**
    ///
    /// カーソルの手前が `\alpha` のような綴りなら記号に替え、そのうしろに
    /// `delim`(打たれた空白や記号)を置く。**替えたら true。**
    /// 替えなければ何もしない — 呼んだ側がふつうに入れる。
    ///
    /// 掛けるのは**区切りを打った時**。打っている途中に替えると、
    /// `\pi` を打とうとして `\p` で止まった人が困る。
    ///
    /// 記号と区切りで**1手**(checkpoint は1回)。替えたことは控えておき、
    /// **直後の Backspace で綴りに戻す** — 「元に戻せること」が
    /// この機能の要件(台帳の札)
    pub fn autocorrect_math(&mut self, delim: &str) -> bool {
        self.autocorrected = None;
        if self.marked.is_some() || self.has_selection() {
            return false; // 変換中と選択中は触らない
        }
        let Some((start, word)) = word_before(&self.text, self.cursor) else {
            return false;
        };
        let Some(sym) = math_symbol(word) else {
            return false;
        };
        let was = word.to_string();
        self.checkpoint();
        self.text.replace_range(start..self.cursor, &format!("{sym}{delim}"));
        self.cursor = start + sym.len() + delim.len();
        self.anchor = self.cursor;
        self.autocorrected = Some(AutoFix {
            caret: self.cursor,
            sym: start..start + sym.len(),
            was,
        });
        true
    }

    /// いま置き換えたばかりの綴り(状態行に「Backspace で戻せます」と出す用)
    pub fn just_autocorrected(&self) -> Option<&str> {
        self.autocorrected
            .as_ref()
            .filter(|a| a.caret == self.cursor)
            .map(|a| a.was.as_str())
    }

    /// 後退削除。選択があればそれを消す。
    ///
    /// **置き換えたばかりの記号の直後なら、記号を綴りに戻す**(消さない)。
    /// 打った区切りはそのまま残る — 消したいのは置き換えであって、
    /// 自分で打った文字ではない(Word と同じ)
    pub fn backspace(&mut self) {
        if let Some(a) = self.autocorrected.take() {
            if a.caret == self.cursor && !self.has_selection() {
                self.checkpoint();
                self.text.replace_range(a.sym.clone(), &a.was);
                let grew = a.was.len() as isize - (a.sym.end - a.sym.start) as isize;
                self.cursor = (self.cursor as isize + grew) as usize;
                self.anchor = self.cursor;
                return;
            }
        }
        if self.has_selection() {
            self.insert("");
            return;
        }
        if self.cursor == 0 {
            return;
        }
        self.checkpoint();
        let p = prev_boundary(&self.text, self.cursor);
        self.text.replace_range(p..self.cursor, "");
        self.cursor = p;
        self.anchor = p;
    }

    pub fn delete(&mut self) {
        self.autocorrected = None;
        if self.has_selection() {
            self.insert("");
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        self.checkpoint();
        let n = next_boundary(&self.text, self.cursor);
        self.text.replace_range(self.cursor..n, "");
    }

    // ---------- IME(未確定文字) ----------

    /// 変換中の文字列を置く。確定していないので undo の区切りは打たない。
    /// `sel` は未確定文字列の中での選択(変換対象の文節)をバイト位置で。
    pub fn set_marked(&mut self, s: &str, sel: Option<Range<usize>>) {
        // 変換の開始時点(まだ何も置いていない状態)を控える
        if self.marked.is_none() {
            self.pre_ime = Some(self.snapshot());
        }
        // 前回の未確定があればそれを、無ければ選択範囲を置き換える
        let r = match self.marked.clone() {
            Some(m) => m,
            None => self.replace_range(),
        };
        self.text.replace_range(r.clone(), s);
        if s.is_empty() {
            self.marked = None;
            self.cursor = r.start;
            self.anchor = r.start;
            return;
        }
        let start = r.start;
        self.marked = Some(start..start + s.len());
        let (a, b) = match sel {
            Some(x) => (start + x.start, start + x.end),
            None => (start + s.len(), start + s.len()),
        };
        self.anchor = clamp_boundary(&self.text, a);
        self.cursor = clamp_boundary(&self.text, b);
    }

    /// 変換を確定する。ここで初めて undo の1手になる。
    ///
    /// 戻り先は**変換が始まる前**の状態。未確定を消した後の状態を控えると、
    /// 選択を置き換える形で始まった変換(選択→変換→確定)で、
    /// undo が「選択していた元の文字列」ではなく空に戻ってしまう。
    pub fn commit_marked(&mut self, s: &str) {
        let before = self.pre_ime.take().unwrap_or_else(|| self.snapshot());
        let r = match self.marked.take() {
            Some(m) => m,
            None => self.replace_range(),
        };
        self.text.replace_range(r.clone(), s);
        self.cursor = r.start + s.len();
        self.anchor = self.cursor;
        if before.text != self.text {
            self.undo.push(before);
            self.redo.clear();
        }
    }

    /// 変換をやめる(未確定を捨てる)。
    pub fn clear_marked(&mut self) {
        self.pre_ime = None;
        if let Some(m) = self.marked.take() {
            self.text.replace_range(m.clone(), "");
            self.cursor = m.start;
            self.anchor = m.start;
        }
    }

    fn replace_range(&self) -> Range<usize> {
        if self.has_selection() {
            self.selection()
        } else {
            self.cursor..self.cursor
        }
    }

    // ---------- undo / redo ----------

    pub fn undo(&mut self) -> bool {
        // 未確定のまま undo されたら、まず変換を捨てる
        if self.marked.is_some() {
            self.clear_marked();
            return true;
        }
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(self.snapshot());
                self.text = prev.text;
                self.cursor = prev.cursor;
                self.anchor = prev.anchor;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(self.snapshot());
                self.text = next.text;
                self.cursor = next.cursor;
                self.anchor = next.anchor;
                true
            }
            None => false,
        }
    }

    // ---------- UTF-16 との境界(IME APIのため) ----------

    pub fn utf16_len(&self) -> usize {
        self.text.chars().map(char::len_utf16).sum()
    }

    /// UTF-16 位置 → バイト位置
    pub fn utf16_to_byte(&self, u: usize) -> usize {
        let mut acc = 0;
        for (b, c) in self.text.char_indices() {
            if acc >= u {
                return b;
            }
            acc += c.len_utf16();
        }
        self.text.len()
    }

    /// バイト位置 → UTF-16 位置
    pub fn byte_to_utf16(&self, b: usize) -> usize {
        let b = b.min(self.text.len());
        self.text[..b].chars().map(char::len_utf16).sum()
    }

    pub fn utf16_range(&self, r: Range<usize>) -> Range<usize> {
        self.byte_to_utf16(r.start)..self.byte_to_utf16(r.end)
    }
    pub fn byte_range(&self, r: Range<usize>) -> Range<usize> {
        self.utf16_to_byte(r.start)..self.utf16_to_byte(r.end)
    }
}

fn clamp_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_boundary(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

fn prev_boundary(s: &str, i: usize) -> usize {
    s[..i].chars().next_back().map_or(0, |c| i - c.len_utf8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_can_be_typed_and_deleted() {
        let mut e = Editor::new("");
        e.insert("サンプル商事");
        assert_eq!(e.text(), "サンプル商事");
        assert_eq!(e.cursor(), "サンプル商事".len());
        e.backspace();
        assert_eq!(e.text(), "サンプル商", "1文字=3バイトを丸ごと消す");
    }

    #[test]
    fn the_cursor_moves_on_character_boundaries() {
        let mut e = Editor::new("あa亜");
        e.move_to(0, false);
        e.move_char(true, false);
        assert_eq!(e.cursor(), 3, "あ は3バイト");
        e.move_char(true, false);
        assert_eq!(e.cursor(), 4, "a は1バイト");
        e.move_char(false, false);
        assert_eq!(e.cursor(), 3);
    }

    #[test]
    fn a_selection_can_be_replaced() {
        let mut e = Editor::new("防火ドア");
        e.move_to(0, false);
        e.move_char(true, true);
        e.move_char(true, true);
        assert_eq!(e.selection(), 0.."防火".len());
        e.insert("玄関");
        assert_eq!(e.text(), "玄関ドア");
    }

    // ---- IME: K2 の難所 ----

    #[test]
    fn ime_preedit_shows_in_the_body_and_commits_as_one_step() {
        let mut e = Editor::new("特定");
        // 「ぼうか」と打つ(未確定)
        e.set_marked("ぼうか", None);
        assert_eq!(e.text(), "特定ぼうか");
        assert_eq!(e.marked_range(), Some(6..6 + "ぼうか".len()));
        // 変換して「防火」に(まだ未確定)
        e.set_marked("防火", None);
        assert_eq!(e.text(), "特定防火", "未確定は置き換わる。積み重ならない");
        // 確定
        e.commit_marked("防火");
        assert_eq!(e.text(), "特定防火");
        assert_eq!(e.marked_range(), None);
        // undo は「防火の確定」を1手として戻す(かな入力の途中には戻らない)
        assert!(e.undo());
        assert_eq!(e.text(), "特定");
    }

    #[test]
    fn ime_cancelling_removes_the_whole_preedit() {
        let mut e = Editor::new("設備");
        e.set_marked("りよう", None);
        assert_eq!(e.text(), "設備りよう");
        e.clear_marked();
        assert_eq!(e.text(), "設備", "変換をやめたら跡が残らない");
        assert_eq!(e.cursor(), "設備".len());
    }

    #[test]
    fn ime_undo_during_preedit_cancels_the_conversion() {
        let mut e = Editor::new("申込");
        e.set_marked("よう", None);
        assert!(e.undo(), "未確定があるときの undo は変換の取り消し");
        assert_eq!(e.text(), "申込");
        assert!(!e.undo(), "それ以上は戻らない");
    }

    #[test]
    fn ime_clause_selection_shows_inside_the_preedit() {
        let mut e = Editor::new("");
        // 「さんぷるしょうじ」→ 変換候補「サンプル商事」、うち「サンプル」が変換対象
        e.set_marked("サンプル商事", Some(0.."サンプル".len()));
        assert_eq!(e.selection(), 0.."サンプル".len());
        assert_eq!(e.marked_range(), Some(0.."サンプル商事".len()));
    }

    #[test]
    fn ime_conversion_starts_by_replacing_the_selection() {
        let mut e = Editor::new("旧製品");
        e.select_all();
        e.set_marked("しんせいひん", None);
        assert_eq!(e.text(), "しんせいひん", "選択は未確定に置き換わる");
        e.commit_marked("新製品");
        assert_eq!(e.text(), "新製品");
        assert!(e.undo());
        assert_eq!(e.text(), "旧製品", "1手で元に戻る");
    }

    #[test]
    fn undo_and_redo_round_trip() {
        let mut e = Editor::new("");
        e.insert("一");
        e.insert("二");
        e.insert("三");
        assert_eq!(e.text(), "一二三");
        assert!(e.undo());
        assert!(e.undo());
        assert_eq!(e.text(), "一");
        assert!(e.redo());
        assert_eq!(e.text(), "一二");
        e.insert("四");
        assert!(!e.redo(), "新しい編集の後は redo が消える");
        assert_eq!(e.text(), "一二四");
    }

    #[test]
    fn utf16_offsets_round_trip() {
        let e = Editor::new("あa𩸽い"); // 𩸽 は UTF-16 で2単位・UTF-8 で4バイト
        assert_eq!(e.utf16_len(), 1 + 1 + 2 + 1);
        for (b, _) in e.text().char_indices().chain([(e.text().len(), ' ')]) {
            let u = e.byte_to_utf16(b);
            assert_eq!(e.utf16_to_byte(u), b, "バイト{b} ⇄ UTF-16 {u} が往復しない");
        }
    }

    #[test]
    fn an_empty_buffer_does_not_break() {
        let mut e = Editor::new("");
        e.backspace();
        e.delete();
        e.move_char(false, false);
        e.move_char(true, false);
        assert_eq!(e.text(), "");
        assert!(!e.undo());
    }

    // ---------- 数学オートコレクト(2026-08-13、台帳)----------

    /// **区切りを打った時に替わる。** 綴りの途中では替わらない
    /// (`\alph` は表に無いので何も起きない)
    #[test]
    fn a_spelling_becomes_a_symbol_when_a_delimiter_is_typed() {
        let mut e = Editor::new("");
        e.insert("\\alph"); // まだ綴りの途中
        assert!(!e.autocorrect_math(" "));
        assert_eq!(e.text(), "\\alph", "途中で替わった");
        e.insert("a");
        assert!(e.autocorrect_math(" "));
        assert_eq!(e.text(), "α ");
        assert_eq!(e.cursor(), e.text().len());
    }

    /// **ふつうの言葉を勝手に置き換えない。** `\` が無ければ相手ではない
    #[test]
    fn spellings_without_a_yen_sign_are_left_alone() {
        let mut e = Editor::new("");
        e.insert("pi");
        assert!(!e.autocorrect_math(" "), "pi が π になった");
        e.insert(" alpha");
        assert!(!e.autocorrect_math(" "));
        assert_eq!(e.text(), "pi alpha");
    }

    /// 知らない綴りは残す(黙って消さない)
    #[test]
    fn a_spelling_not_in_the_table_stays() {
        let mut e = Editor::new("");
        e.insert("\\nosuch");
        assert!(!e.autocorrect_math(" "));
        assert_eq!(e.text(), "\\nosuch");
    }

    /// **元に戻せることが要件**(台帳の札)。直後の Backspace で綴りに戻り、
    /// 自分で打った区切りは残る
    #[test]
    fn backspace_right_after_returns_to_the_spelling() {
        let mut e = Editor::new("");
        e.insert("x=\\ne");
        assert!(e.autocorrect_math("y"));
        assert_eq!(e.text(), "x=≠y");
        assert_eq!(e.just_autocorrected(), Some("\\ne"));
        e.backspace();
        assert_eq!(e.text(), "x=\\ney", "綴りに戻らない");
        assert_eq!(e.cursor(), e.text().len());
        // 2回目はふつうの後退削除
        e.backspace();
        assert_eq!(e.text(), "x=\\ne");
    }

    /// Ctrl+Z(Editor の undo)でも1手で戻る — 記号と区切りで1手
    #[test]
    fn cancelling_is_also_one_step() {
        let mut e = Editor::new("");
        e.insert("\\times");
        assert!(e.autocorrect_math(" "));
        assert_eq!(e.text(), "× ");
        assert!(e.undo());
        assert_eq!(e.text(), "\\times");
    }

    /// 別の打鍵をしたら控えは捨てる — 位置がたまたま戻っても吐き出さない
    #[test]
    fn backspace_after_typing_deletes_normally() {
        let mut e = Editor::new("");
        e.insert("\\pi");
        assert!(e.autocorrect_math(" "));
        e.insert("r");
        assert_eq!(e.text(), "π r");
        e.backspace();
        assert_eq!(e.text(), "π ", "綴りが吐き出された");
    }

    #[test]
    fn the_table_matches_the_spelling_exactly() {
        assert_eq!(math_symbol("\\alpha"), Some("α"));
        assert_eq!(math_symbol("\\alphabet"), None);
        assert_eq!(math_symbol("alpha"), None);
        assert_eq!(math_symbol("\\Omega"), Some("Ω"));
        assert_eq!(math_symbol("\\omega"), Some("ω"), "大小を取り違えている");
    }
}
