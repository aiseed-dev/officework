//! The ribbon settings file (`<config dir>/ribbon.toml`).
//!
//! A dedicated edition (for example, one only for resumes and application
//! forms) is the same app with this file: it names the app, picks the tabs
//! and their order, and picks the buttons on a tab. Without the file the
//! ribbon is the full one.
//!
//! ```toml
//! name = "aiseed office"
//! tabs = "Documents, Home, Check"
//! buttons.Documents = "open, pdf"
//! ```
//!
//! The File tab is always first and keeps its page; `tabs` lists the tabs
//! after it. A tab name is matched against the English name, the Japanese
//! name and the current language's name; any other name makes a new tab
//! that holds only the buttons listed for it and the user's Python buttons
//! whose declared tab has that name. Buttons are named by their id and may
//! come from any tab.
//!
//! The file is read once, by [`load`] at start-up, before the ribbon is
//! first built. Tests never call [`load`], so they always see the full
//! ribbon.

use std::sync::OnceLock;

/// What the settings file says. Empty fields mean "as without the file".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// The app name for the window title and the app menu
    pub name: Option<String>,
    /// The tabs after File, in order. `None` keeps all tabs
    pub tabs: Option<Vec<String>>,
    /// Button ids per tab name, in order
    pub buttons: Vec<(String, Vec<String>)>,
}

static PROFILE: OnceLock<Option<Profile>> = OnceLock::new();

/// The settings file's place
pub fn path() -> std::path::PathBuf {
    lang::config_dir().join("ribbon.toml")
}

/// Read the settings file once. Call it before the ribbon is first built.
/// Returns the profile, or `None` when there is no file
pub fn load() -> Option<&'static Profile> {
    PROFILE
        .get_or_init(|| {
            let s = std::fs::read_to_string(path()).ok()?;
            let p = parse(&s);
            (p != Profile::default()).then_some(p)
        })
        .as_ref()
}

/// The loaded profile (`None` before [`load`] or without a file)
pub fn get() -> Option<&'static Profile> {
    PROFILE.get().and_then(|p| p.as_ref())
}

/// The app name: the profile's, or "officework"
pub fn app_name() -> &'static str {
    get().and_then(|p| p.name.as_deref()).unwrap_or("officework")
}

fn list(v: &str) -> Vec<String> {
    v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Read `key = "value"` lines (the same plain form as settings.toml)
pub fn parse(s: &str) -> Profile {
    let mut p = Profile::default();
    for line in s.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        if k == "name" {
            if !v.trim().is_empty() {
                p.name = Some(v.trim().to_string());
            }
        } else if k == "tabs" {
            p.tabs = Some(list(v));
        } else if let Some(tab) = k.strip_prefix("buttons.") {
            p.buttons.push((tab.trim().trim_matches('"').to_string(), list(v)));
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon;

    #[test]
    fn reads_the_name_tabs_and_buttons() {
        let p = parse(
            "# comment\nname = \"aiseed office\"\ntabs = \"Documents, Home ,Check\"\nbuttons.Documents = \"open, pdf\"\n",
        );
        assert_eq!(p.name.as_deref(), Some("aiseed office"));
        assert_eq!(p.tabs, Some(vec!["Documents".into(), "Home".into(), "Check".into()]));
        assert_eq!(p.buttons, vec![("Documents".into(), vec!["open".into(), "pdf".into()])]);
        assert_eq!(parse(""), Profile::default());
    }

    #[test]
    fn picks_tabs_and_buttons_and_keeps_file_first() {
        let full = ribbon::base();
        let p = parse(
            "tabs = \"Documents, Home, File, Check\"\nbuttons.Documents = \"open, pdf, no-such-id\"\nbuttons.Home = \"paste, copy\"\n",
        );
        let t = ribbon::apply_profile(&p, full);
        let names: Vec<&str> = t.iter().map(|t| t.name).collect();
        assert_eq!(names, ["File", "Documents", "Home", "Check"]);
        assert_eq!(t[0].cmds.len(), full[0].cmds.len(), "File keeps its buttons");
        let ids = |i: usize| t[i].cmds.iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids(1), ["open", "pdf"], "unknown ids are skipped");
        assert_eq!(ids(2), ["paste", "copy"]);
        // A Japanese tab name finds the same tab
        let ja = ribbon::apply_profile(&parse("tabs = \"ホーム\""), full);
        assert_eq!(ja[1].name, "Home");
        assert!(t[3].cmds.is_empty(), "a new tab without buttons holds only Python buttons");
    }

    #[test]
    fn without_tabs_all_tabs_stay_and_only_listed_buttons_change() {
        let full = ribbon::base();
        let t = ribbon::apply_profile(&parse("buttons.Home = \"copy\""), full);
        assert_eq!(t.len(), full.len());
        assert_eq!(t[1].cmds.iter().map(|c| c.id).collect::<Vec<_>>(), ["copy"]);
        assert_eq!(t[2].cmds.len(), full[2].cmds.len());
        // Only a name: the ribbon is the full one
        assert!(std::ptr::eq(ribbon::apply_profile(&parse("name = \"x\""), full), full));
    }
}
