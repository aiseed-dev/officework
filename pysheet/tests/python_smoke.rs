//! Python 側から往復できることの検査。
//!
//! cdylib を組み、`officework/_sheet.so` の名前で置いて、`test.py` を回す。
//! **配る wheel と同じ形** — 平場ではなく officework の副モジュール。
//! python3 が無い機械では飛ばす(無いのに失敗と言わない)。
use std::process::Command;

#[test]
fn python側から帳票を差し込める() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("python3 が無いので飛ばす");
        return;
    }

    // この試験自体のビルドでは cdylib が出来ているとは限らないので、組む。
    // (外の cargo のビルドは終わっているので、ここで cargo を呼んでも詰まらない)
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "pysheet"])
        .current_dir(root)
        .status()
        .expect("cargo を呼べない");
    assert!(status.success(), "pysheet が組めない");

    // target/debug の場所は、この試験の実行ファイルから辿る
    // (target/debug/deps/xxx → target/debug)
    let exe = std::env::current_exe().expect("自分の場所が分からない");
    let debug = exe.parent().and_then(|p| p.parent()).expect("target/debug が見つからない");
    let so = debug.join("lib_sheet.so");
    assert!(so.exists(), "{} が無い", so.display());

    // Python の import 名に合わせて置く
    let dir = debug.join("pysheet-import");
    std::fs::create_dir_all(&dir).expect("作業場所を作れない");
    // **ソースの officework/ の隣に置く。** 一時ディレクトリに組んでも、
    // .venv の officework.pth がソースの方を先に掴むので負ける(2026-08-09 に踏んだ)。
    // 終わったら消す
    let pkg = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/officework"));
    let ext = pkg.join("_sheet.so");
    std::fs::copy(&so, &ext).expect("置けない");
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(ext);

    // xlsx(sheet)と docx(doc)。**同じ .so で両方**が動くことまで見る —
    // 1つの wheel に同居させているので、片方だけ通っても足りない。
    // test_gokan.py は互換層の適合検査 — openpyxl / python-docx が居る環境では
    // 本家と結果を突き合わせ、居なければその節を飛ばしたと言って通る。
    // test_shiyou.py は**本家の受け入れ仕様から起こした検査**(本家が居なくても回る)
    for script in ["test.py", "test_doc.py", "test_gokan.py", "test_shiyou.py", "test_tex.py"] {
        let out = Command::new("python3")
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(script))
            .env("PYTHONPATH", &dir)
            .output()
            .expect("python3 を回せない");
        assert!(
            out.status.success(),
            "{script} が失敗:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
