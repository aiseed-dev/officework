//! **キーの割り当ての正本**(gpui を通らない)。
//!
//! 表そのものと、既定と上書きを合わせる芯([`compose_keys`])を持つ。
//! gpui の `KeyBinding` を組み立てる `ui::bindings_for` は `ui` に残る —
//! **組み立てはアプリの仕事、何をどの鍵に割り当てるかはこちらの仕事**。
//!
//! Kotlin / Swift のアプリもこの表を読む。だから
//! 「鍵の書き方が読めるかどうか」の判定は [`compose_keys`] に**引数で
//! 渡す** — gpui のアプリは `gpui::Keystroke::parse` を、他のアプリはそれぞれの
//! 読み手を渡す。ここで gpui を呼んでいたのを 2026-08-15 に引数へ出した。

/// 既定の割り当ての表(鍵, 操作名)。**この表が正本** — 束縛は
/// [`bindings_for`] がここから作り、settings.toml の `key.操作名 = "鍵"` が
/// 上書きし、tools/keys_check.py が手引きの表との揃いを見る。
///
/// 同じ操作に行が2つある物(ctrl-f と ctrl-h の Find など)はどちらも効く。
/// **受け口の無いアプリに束縛を作らない** — 前は1本の表を両アプリに配り、
/// 「束縛はあるが writer では動かない」鍵があった(sugata の部屋
/// 「キーの嘘」)。表を 共通/calc/writer に割って、その状態を無くした
/// (2026-08-14)
pub const KEYS_COMMON: &[(&str, &str)] = &[
    ("backspace", "Backspace"),
    ("delete", "Delete"),
    ("left", "Left"),
    ("right", "Right"),
    ("shift-left", "SelectLeft"),
    ("shift-right", "SelectRight"),
    ("ctrl-left", "WordLeft"),
    ("ctrl-right", "WordRight"),
    ("ctrl-shift-left", "SelectWordLeft"),
    ("ctrl-shift-right", "SelectWordRight"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
    ("ctrl-f", "Find"),
    // Ctrl+H(本家の「検索と置換」)も同じ口へ — ここのパネルは
    // 探す言葉 → 置き換える言葉 の2段で、空なら検索だけ
    ("ctrl-h", "Find"),
    ("ctrl-b", "Bold"),
    ("ctrl-i", "Italic"),
    ("ctrl-u", "Underline"),
    ("ctrl-5", "Strikeout"),
    ("ctrl-p", "Print"),
    ("f11", "FullScreen"),
    ("ctrl-shift-s", "SaveAs"),
    // F12 も名前を付けて保存(本家と同じ。2026-08-14 に追加)
    ("f12", "SaveAs"),
    ("ctrl-0", "ZoomReset"),
    ("f1", "Help"),
    ("ctrl-;", "InsDate"),
    ("ctrl-:", "InsTime"),
    ("ctrl-home", "DocHome"),
    ("ctrl-end", "DocEnd"),
    ("shift-up", "SelectUp"),
    ("shift-down", "SelectDown"),
    ("ctrl-a", "SelectAll"),
    ("home", "Home"),
    ("end", "End"),
    ("enter", "Enter"),
    ("up", "Up"),
    ("down", "Down"),
    ("tab", "Tab"),
    ("shift-tab", "ShiftTab"),
    ("ctrl-z", "Undo"),
    ("ctrl-shift-z", "Redo"),
    ("ctrl-y", "Redo"),
    ("ctrl-s", "Save"),
    ("ctrl-o", "Open"),
    ("ctrl-c", "Copy"),
    ("ctrl-x", "Cut"),
    ("ctrl-v", "Paste"),
    ("ctrl-q", "Quit"),
    ("menu", "ContextMenu"),
    ("shift-f10", "ContextMenu"),
    ("ctrl-=", "UiBigger"),
    ("ctrl-shift-=", "UiBigger"),
    ("ctrl--", "UiSmaller"),
    ("ctrl-k", "InsLink"),
    ("escape", "Cancel"),
];

/// calc だけの割り当て(受け口が calc にしか無い物)
pub const KEYS_CALC: &[(&str, &str)] = &[
    ("ctrl-up", "EdgeUp"),
    ("ctrl-down", "EdgeDown"),
    ("ctrl-shift-up", "SelectEdgeUp"),
    ("ctrl-shift-down", "SelectEdgeDown"),
    ("f2", "EditCell"),
    ("shift-f3", "InsertFn"),
    ("ctrl-shift-%", "PercentFmt"),
    ("ctrl-e", "FlashFill"),
    ("ctrl-shift-v", "PasteValues"),
    ("ctrl-shift-enter", "ArrayEnter"),
    ("f9", "Recalc"),
    ("shift-f9", "RecalcSheet"),
    ("alt-enter", "NewLine"),
    ("alt-pageup", "PrevSheet"),
    ("alt-pagedown", "NextSheet"),
    // 本家の鍵(2026-08-14 に追加)。Alt 版も当面残す — 衝突しない
    ("ctrl-pageup", "PrevSheet"),
    ("ctrl-pagedown", "NextSheet"),
    ("f4", "CycleRef"),
    ("alt-s", "SlicerMulti"),
    ("alt-c", "SlicerClear"),
    // ここから 2026-08-14 の増強(本家の定番)
    ("ctrl-1", "CellFormat"),
    ("ctrl-space", "SelectCol"),
    ("shift-space", "SelectRow"),
    ("alt-=", "AutoSum"),
    ("ctrl-d", "FillDown"),
    ("ctrl-r", "FillRight"),
    ("ctrl-g", "Jump"),
    ("f5", "Jump"),
    ("ctrl-shift-l", "ToggleFilter"),
    ("ctrl-t", "MakeTable"),
    ("shift-f2", "AddComment"),
];

/// writer だけの割り当て。ctrl-e は calc ではフラッシュフィル、
/// writer では中央揃え — **本家の手の記憶がアプリごとに違う**ので、
/// 同じ鍵でも表を分けて別の操作に割り当てる
pub const KEYS_WRITER: &[(&str, &str)] = &[
    ("ctrl-e", "AlignCenter"),
    ("ctrl-l", "AlignLeft"),
    ("ctrl-r", "AlignRight"),
    ("ctrl-j", "AlignJustify"),
    ("ctrl-enter", "PageBreak"),
    ("ctrl-]", "FontBigger"),
    ("ctrl-[", "FontSmaller"),
];

/// アプリの既定の表(共通+アプリ固有)。マニュアル生成の道具も
/// この並びを読む
pub fn default_keys(app: &str) -> Vec<(&'static str, &'static str)> {
    let own: &[(&str, &str)] = match app {
        "calc" => KEYS_CALC,
        "writer" => KEYS_WRITER,
        _ => &[],
    };
    KEYS_COMMON.iter().chain(own).copied().collect()
}

/// 合成で見つけた言い分。**翻訳は掛けない**(bindings_for が最後に
/// 掛ける)— 芯を言語から切り離し、試験が文言に依らないようにする
#[derive(Debug, PartialEq)]
pub enum KeyWarn {
    /// 知らない操作名
    UnknownAction(String),
    /// 読めない鍵(操作名, 鍵)
    BadKey(String, String),
    /// 同じ鍵の取り合い(鍵, 先の操作, 後の操作 — 後が勝つ)
    Contested(String, String, String),
}

/// 既定の表と上書きの**合成の芯**(純関数 — 試験がここを直に叩く)。
///
/// 決め(2026-08-14): 名前の照合は大文字小文字を見ない。1つの操作に
/// 複数の鍵は「,」区切り。上書きは**その操作の既定の鍵を全部置き換える**。
/// 空文字なら外す。知らない名前・読めない鍵は**その行だけ捨てて、
/// 言い分に残す**。取り合い(同じ鍵に別の操作)は後の者が勝ち、それも言う
///
/// `readable` は**鍵の書き方が読めるか**の判定。gpui のアプリは
/// `gpui::Keystroke::parse(part).is_ok()` を渡す。2026-08-15 に gpui の
/// 直呼びからここへ出した — この関数をアプリから出すため。鍵の書き方の
/// 決まりはアプリごとに違うので、**判定を持たずに受け取るのが正しい**
pub fn compose_keys(
    defaults: &[(&str, &str)],
    overrides: &[(String, String)],
    known: &dyn Fn(&str) -> bool,
    readable: &dyn Fn(&str) -> bool,
) -> (Vec<(String, String)>, Vec<KeyWarn>) {
    let mut rows: Vec<(String, String)> = defaults
        .iter()
        .map(|(k, n)| (k.to_string(), n.to_string()))
        .collect();
    let mut warns: Vec<KeyWarn> = Vec::new();
    for (name, keys) in overrides {
        if !known(name) {
            warns.push(KeyWarn::UnknownAction(name.clone()));
            continue;
        }
        let mut good: Vec<String> = Vec::new();
        for key in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let ok = key.split_whitespace().all(readable);
            if ok {
                good.push(key.to_string());
            } else {
                warns.push(KeyWarn::BadKey(name.clone(), key.to_string()));
            }
        }
        // 空文字は「外す」の意思。読める鍵が1つも無い書き損じなら
        // **既定を残す** — 書き間違い1つで鍵が全部消えるのは酷
        if good.is_empty() && !keys.trim().is_empty() {
            continue;
        }
        rows.retain(|(_, n)| !n.eq_ignore_ascii_case(name));
        rows.extend(good.into_iter().map(|k| (k, name.clone())));
    }
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].0 == rows[j].0 && !rows[i].1.eq_ignore_ascii_case(&rows[j].1) {
                warns.push(KeyWarn::Contested(
                    rows[i].0.clone(), rows[i].1.clone(), rows[j].1.clone(),
                ));
            }
        }
    }
    (rows, warns)
}

#[cfg(test)]
mod key_tests {
    use super::compose_keys;

    /// 鍵の書き方が読めるか(**試験用の写し**)。本物はアプリが渡す —
    /// gpui なら `gpui::Keystroke::parse`。ここでは同じ決まりを
    /// 小さく写して、芯の筋道だけを見る:最後が鍵で手前は修飾キー
    fn readable(part: &str) -> bool {
        let mut it = part.split('-').peekable();
        while let Some(w) = it.next() {
            if it.peek().is_none() {
                return !w.is_empty();
            }
            if !matches!(w, "ctrl" | "alt" | "shift" | "cmd" | "fn" | "secondary") {
                return false;
            }
        }
        false
    }

    fn known(n: &str) -> bool {
        ["Bold", "Italic", "Find"].iter().any(|k| k.eq_ignore_ascii_case(n))
    }
    fn over(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn an_override_replaces_one_key_and_leaves_the_rest() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-f", "Find"), ("ctrl-h", "Find")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "alt-b")]), &known, &readable);
        assert!(rows.contains(&("alt-b".into(), "bold".into())), "{rows:?}");
        assert!(!rows.iter().any(|(k, _)| k == "ctrl-b"));
        assert_eq!(rows.iter().filter(|(_, n)| n == "Find").count(), 2);
        assert!(warns.is_empty(), "{warns:?}");
    }

    #[test]
    fn an_unknown_action_is_reported_and_defaults_stay() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("nosuch", "ctrl-x")]), &known, &readable);
        assert_eq!(warns, vec![super::KeyWarn::UnknownAction("nosuch".into())]);
        assert!(rows.contains(&("ctrl-b".into(), "Bold".into())));
    }

    #[test]
    fn an_unreadable_override_keeps_defaults_and_says_so() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) =
            compose_keys(&defaults, &over(&[("bold", "nosuchmod-b")]), &known, &readable);
        assert_eq!(
            warns,
            vec![super::KeyWarn::BadKey("bold".into(), "nosuchmod-b".into())]
        );
        // 書き損じで鍵が消えたら酷 — 既定の ctrl-b は生きている
        assert!(rows.contains(&("ctrl-b".into(), "Bold".into())), "{rows:?}");
    }

    #[test]
    fn an_empty_override_removes_the_key() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-i", "Italic")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "")]), &known, &readable);
        assert!(warns.is_empty(), "{warns:?}");
        assert!(!rows.iter().any(|(_, n)| n.eq_ignore_ascii_case("bold")));
        assert!(rows.iter().any(|(_, n)| n == "Italic"));
    }

    #[test]
    fn a_contested_key_is_reported_and_the_later_one_wins() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-i", "Italic")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("find", "ctrl-b")]), &known, &readable);
        assert_eq!(
            warns,
            vec![super::KeyWarn::Contested("ctrl-b".into(), "Bold".into(), "find".into())]
        );
        // 後の者(find)の行が表の後ろに居る — GPUI は後から結んだ方を優先
        let bold_at = rows.iter().position(|(k, n)| k == "ctrl-b" && n == "Bold").unwrap();
        let find_at = rows.iter().position(|(k, n)| k == "ctrl-b" && n == "find").unwrap();
        assert!(find_at > bold_at);
    }

    #[test]
    fn several_keys_can_be_listed_with_commas() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "alt-b, ctrl-shift-b")]), &known, &readable);
        assert!(warns.is_empty(), "{warns:?}");
        assert!(rows.contains(&("alt-b".into(), "bold".into())));
        assert!(rows.contains(&("ctrl-shift-b".into(), "bold".into())));
    }
}
