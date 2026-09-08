//! キーを端末のバイト列にする(Zed の terminal/src/mappings/keys.rs の蒸留)。
//!
//! 普通の字は gpui が `key_char` で渡すので、ここは制御の字と矢印などの
//! エスケープ列だけを持ちます。`app_cursor` は端末が DECCKM(アプリの
//! カーソルキー)に入っているかで、vim などが要ります。

use gpui::Keystroke;

/// 修飾キーの組(Zed と同じ分け方)
#[derive(Debug, PartialEq, Eq)]
enum Mods {
    None,
    Alt,
    Ctrl,
    Shift,
    CtrlShift,
    Other,
}

impl Mods {
    fn of(ks: &Keystroke) -> Self {
        match (ks.modifiers.alt, ks.modifiers.control, ks.modifiers.shift, ks.modifiers.platform) {
            (false, false, false, false) => Mods::None,
            (true, false, false, false) => Mods::Alt,
            (false, true, false, false) => Mods::Ctrl,
            (false, false, true, false) => Mods::Shift,
            (false, true, true, false) => Mods::CtrlShift,
            _ => Mods::Other,
        }
    }
}

/// キー → 端末へ送る字。普通の字(`key_char`)は呼ぶ側が先に見る
pub fn to_esc(ks: &Keystroke, app_cursor: bool) -> Option<String> {
    let m = Mods::of(ks);
    let cursor = |app: &'static str, normal: &'static str| -> Option<&'static str> { Some(if app_cursor { app } else { normal }) };
    let fixed: Option<&str> = match (ks.key.as_str(), &m) {
        ("tab", Mods::None) => Some("\x09"),
        ("tab", Mods::Shift) => Some("\x1b[Z"),
        ("escape", Mods::None) => Some("\x1b"),
        ("enter", Mods::None) => Some("\x0d"),
        ("enter", Mods::Shift) => Some("\x0a"),
        ("enter", Mods::Alt) => Some("\x1b\x0d"),
        ("backspace", Mods::None) | ("backspace", Mods::Shift) => Some("\x7f"),
        ("backspace", Mods::Ctrl) => Some("\x08"),
        ("backspace", Mods::Alt) => Some("\x1b\x7f"),
        ("space", Mods::Ctrl) => Some("\x00"),
        ("home", Mods::None) => cursor("\x1bOH", "\x1b[H"),
        ("end", Mods::None) => cursor("\x1bOF", "\x1b[F"),
        ("up", Mods::None) => cursor("\x1bOA", "\x1b[A"),
        ("down", Mods::None) => cursor("\x1bOB", "\x1b[B"),
        ("right", Mods::None) => cursor("\x1bOC", "\x1b[C"),
        ("left", Mods::None) => cursor("\x1bOD", "\x1b[D"),
        ("insert", Mods::None) => Some("\x1b[2~"),
        ("delete", Mods::None) => Some("\x1b[3~"),
        ("pageup", Mods::None) => Some("\x1b[5~"),
        ("pagedown", Mods::None) => Some("\x1b[6~"),
        ("f1", Mods::None) => Some("\x1bOP"),
        ("f2", Mods::None) => Some("\x1bOQ"),
        ("f3", Mods::None) => Some("\x1bOR"),
        ("f4", Mods::None) => Some("\x1bOS"),
        ("f5", Mods::None) => Some("\x1b[15~"),
        ("f6", Mods::None) => Some("\x1b[17~"),
        ("f7", Mods::None) => Some("\x1b[18~"),
        ("f8", Mods::None) => Some("\x1b[19~"),
        ("f9", Mods::None) => Some("\x1b[20~"),
        ("f10", Mods::None) => Some("\x1b[21~"),
        ("f11", Mods::None) => Some("\x1b[23~"),
        ("f12", Mods::None) => Some("\x1b[24~"),
        _ => None,
    };
    if let Some(s) = fixed {
        return Some(s.to_string());
    }
    // Ctrl + 字: a〜z は 1〜26、記号は表のとおり
    if matches!(m, Mods::Ctrl | Mods::CtrlShift) {
        let k = ks.key.as_str();
        if k.len() == 1 {
            let c = k.chars().next().unwrap().to_ascii_lowercase();
            let code = match c {
                'a'..='z' => Some(c as u8 - b'a' + 1),
                '@' => Some(0),
                '[' => Some(0x1b),
                '\\' => Some(0x1c),
                ']' => Some(0x1d),
                '^' => Some(0x1e),
                '_' => Some(0x1f),
                '?' => Some(0x7f),
                _ => None,
            };
            if let Some(b) = code {
                return Some((b as char).to_string());
            }
        }
    }
    if m != Mods::None {
        let code = modifier_code(ks);
        let s = match ks.key.as_str() {
            "up" => format!("\x1b[1;{code}A"),
            "down" => format!("\x1b[1;{code}B"),
            "right" => format!("\x1b[1;{code}C"),
            "left" => format!("\x1b[1;{code}D"),
            "home" => format!("\x1b[1;{code}H"),
            "end" => format!("\x1b[1;{code}F"),
            "insert" => format!("\x1b[2;{code}~"),
            "delete" => format!("\x1b[3;{code}~"),
            "pageup" => format!("\x1b[5;{code}~"),
            "pagedown" => format!("\x1b[6;{code}~"),
            _ => String::new(),
        };
        if !s.is_empty() {
            return Some(s);
        }
    }
    // Alt + 字 = ESC 字(mac は Option を字に使うので送らない)
    if !cfg!(target_os = "macos") && ks.modifiers.alt && !ks.modifiers.control && ks.key.is_ascii() && ks.key.len() == 1 {
        let k = if ks.modifiers.shift { ks.key.to_ascii_uppercase() } else { ks.key.clone() };
        return Some(format!("\x1b{k}"));
    }
    None
}

fn modifier_code(ks: &Keystroke) -> u32 {
    let mut c = 0;
    if ks.modifiers.shift {
        c |= 1;
    }
    if ks.modifiers.alt {
        c |= 2;
    }
    if ks.modifiers.control {
        c |= 4;
    }
    c + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn ks(key: &str, control: bool, alt: bool, shift: bool) -> Keystroke {
        Keystroke {
            modifiers: Modifiers { control, alt, shift, platform: false, function: false },
            key: key.to_string(),
            key_char: None,
        }
    }

    #[test]
    fn arrows_follow_the_cursor_key_mode_and_ctrl_letters_become_control_codes() {
        assert_eq!(to_esc(&ks("up", false, false, false), false).unwrap(), "\x1b[A");
        assert_eq!(to_esc(&ks("up", false, false, false), true).unwrap(), "\x1bOA");
        assert_eq!(to_esc(&ks("c", true, false, false), false).unwrap(), "\x03");
        assert_eq!(to_esc(&ks("enter", false, false, false), false).unwrap(), "\r");
        assert_eq!(to_esc(&ks("up", true, false, false), false).unwrap(), "\x1b[1;5A");
        assert_eq!(to_esc(&ks("a", false, false, false), false), None, "普通の字は key_char で送る");
    }
}
