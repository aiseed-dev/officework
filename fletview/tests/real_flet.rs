//! **Against the real Flet.** A Flet program (Python) is started with
//! `FLET_VIEW_PATH` pointing at a script that, instead of a Flutter window,
//! writes the page URL to a file and waits. This crate then connects as the
//! client: it registers, receives the control tree, clicks the button and
//! sees the text change. This is where a new Flet version shows what it
//! changed in the protocol.
//!
//! Needs a Python with `flet` (the one named by `FLET_PYTHON`, else
//! `python3`). Without one the test says so and passes; it is not a failure
//! of this crate. Unix only (the stand-in client is a shell script).

#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use fletview::client::page_props;
use fletview::{Client, Incoming, Value};

const APP: &str = r#"
import flet as ft

def main(page: ft.Page):
    page.title = "fletview test"
    t = ft.Text("押す前")
    def click(e):
        t.value = "押した"
        page.update()
    page.add(ft.Column([t, ft.Button("押す", on_click=click), ft.TextField(label="名前")]))

ft.run(main)
"#;

fn python_with_flet() -> Option<String> {
    let py = std::env::var("FLET_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let ok = Command::new(&py)
        .args(["-c", "import flet"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    ok.then_some(py)
}

#[test]
fn a_real_flet_program_registers_patches_and_answers_a_click() {
    let Some(py) = python_with_flet() else {
        eprintln!("flet の入った Python が無いので、この検査は飛ばした(FLET_PYTHON で指定できます)");
        return;
    };
    let dir: PathBuf = std::env::temp_dir().join(format!("fletview-test-{}", std::process::id()));
    let view = dir.join("view");
    fs::create_dir_all(&view).unwrap();
    fs::write(dir.join("app.py"), APP).unwrap();
    let url_file = dir.join("url");
    // The stand-in client: tell us the URL, write the pid Flet waits for, stay alive
    fs::write(
        view.join("flet"),
        format!(
            "#!/bin/sh\necho \"$1\" > \"{}\"\necho $$ > \"$2\"\necho $$ > \"{}\"\nexec sleep 120\n",
            url_file.display(),
            dir.join("view.pid").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(view.join("flet"), fs::Permissions::from_mode(0o755)).unwrap();

    let mut app = Command::new(&py)
        .arg(dir.join("app.py"))
        .current_dir(&dir)
        .env("FLET_VIEW_PATH", &view)
        .stdout(fs::File::create(dir.join("out.txt")).unwrap())
        .stderr(fs::File::create(dir.join("err.txt")).unwrap())
        .spawn()
        .expect("Python が起動できない");

    let result = std::panic::catch_unwind(|| {
        let start = Instant::now();
        let url = loop {
            if let Ok(u) = fs::read_to_string(&url_file) {
                let u = u.trim().to_string();
                if !u.is_empty() {
                    break u;
                }
            }
            assert!(start.elapsed() < Duration::from_secs(20), "Flet が画面側を起動しませんでした");
            std::thread::sleep(Duration::from_millis(100));
        };
        let mut c = Client::connect(&url).expect("つなげない");
        let r = c.register(page_props(800.0, 600.0, "linux")).expect("登録できない");
        assert!(r.session_id.is_some());
        let root = c.tree.root.expect("Page が無い");
        assert_eq!(c.tree.get(root).unwrap().ctype, "Page");

        let mut clicked = false;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "待ちきれませんでした");
            match c.next().expect("受け取れない") {
                Incoming::Patched { .. } => {
                    if !clicked {
                        if let Some(b) = c.tree.find_type("Button") {
                            assert!(b.has_event("click"));
                            let id = b.id;
                            c.send_event(id, "click", Value::Nil).unwrap();
                            clicked = true;
                        }
                    } else if c.tree.find_type("Text").is_some_and(|t| t.get_str("value") == Some("押した")) {
                        break;
                    }
                }
                Incoming::Crashed(m) => panic!("Python が落ちました: {m}"),
                _ => {}
            }
        }
        assert_eq!(c.tree.get(root).unwrap().get_str("title"), Some("fletview test"));
        assert!(c.tree.find_type("TextField").is_some());
    });

    let _ = app.kill();
    let _ = app.wait();
    // the stand-in client sleeps; stop it too
    for f in fs::read_dir(&dir).into_iter().flatten().flatten() {
        if f.path().extension().is_some_and(|e| e == "pid") {
            if let Ok(pid) = fs::read_to_string(f.path()) {
                let _ = Command::new("kill").arg(pid.trim()).status();
            }
        }
    }
    let _ = fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
