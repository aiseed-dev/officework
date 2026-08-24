//! Python 側から往復できることの検査。
//!
//! cdylib を組み、Python が読む名前で置いて、`test.py` を回す。
//! **配る wheel と同じ形** — 平場ではなく officework の副モジュール。
//! Python が無い機械では飛ばす(無いのに失敗と言わない)。
//!
//! **OS ごとに違う所が3つあります**(2026-08-22 に3 OS の CI で分かった)。
//!
//! * 組み上がる名前 — Linux `lib_sheet.so` / mac `lib_sheet.dylib` /
//!   Windows `_sheet.dll`(頭に `lib` が付かない)
//! * 置く名前 — Linux と mac は `.so`、Windows は `.pyd`(CPython の作法)
//! * Python の呼び名 — Windows に `python3` は無いことがあり、`python`
//!   か `py` になります
//!
//! 名前は `rustc --target <対象> --print file-names --crate-type cdylib`
//! で確かめました(推測ではありません)。
//!
//! **試験は写しの上で回します**(2026-08-24)。前はソースの `officework/` に
//! `.so` を置いていましたが、開発機に `_sheet.abi3.so`(extension-module で
//! 手組みした物)が残っていると、CPython はそちらを先に読みます —
//! 拡張子の探索順が `.abi3.so` → `.so` の順だからです。つまり**組み立てた
//! ばかりの物ではなく、古い物を試験して緑になっていました**。
//! `target/debug/pysheet-stage` にパッケージとスクリプトを写し、そこから
//! 回せば、読む物が1つに決まります。スクリプトを写すのは、CPython が
//! スクリプトの置き場を `sys.path` の先頭に入れるためです(PYTHONPATH より
//! 強いので、置き場ごと写すしかありません)。
use std::process::Command;

#[test]
fn python側から帳票を差し込める() {
    // **Python の名前は OS で違う。** Windows は `python`(と `py`)で、
    // `python3` は無いことがあります。無い名前で呼ぶと「回せない」で
    // 落ちるので、在る物を探します
    let Some(py) = ["python3", "python", "py"]
        .into_iter()
        .find(|n| {
            Command::new(n)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        })
    else {
        eprintln!("Python が無いので飛ばす");
        return;
    };

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
    // **OS ごとに名前が違う。** cargo が出す cdylib の名前:
    //   Linux   lib_sheet.so
    //   mac     lib_sheet.dylib
    //   Windows _sheet.dll(頭に lib が付かない)
    let 組んだ名 = if cfg!(target_os = "windows") {
        "_sheet.dll"
    } else if cfg!(target_os = "macos") {
        "lib_sheet.dylib"
    } else {
        "lib_sheet.so"
    };
    let so = debug.join(組んだ名);
    assert!(so.exists(), "{} が無い", so.display());

    // 写しの作業場所。前の回の残りが混ざらないよう、毎回作り直す
    let stage = debug.join("pysheet-stage");
    if stage.exists() {
        std::fs::remove_dir_all(&stage).expect("前の写しを消せない");
    }
    let pkg_dst = stage.join("officework");
    std::fs::create_dir_all(&pkg_dst).expect("写しの場所を作れない");

    // パッケージの .py を写す(__pycache__ などは要らない)
    let pkg_src = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/officework"));
    for entry in std::fs::read_dir(&pkg_src).expect("officework/ を読めない") {
        let path = entry.expect("officework/ を読めない").path();
        if path.extension().is_some_and(|e| e == "py") {
            let name = path.file_name().expect("名前が無い");
            std::fs::copy(&path, pkg_dst.join(name)).expect("py を写せない");
        }
    }
    // **Python が探す名前も OS で違う。** Windows は `.pyd`、
    // mac は `.dylib` ではなく `.so`(CPython の作法)
    let 置く名 = if cfg!(target_os = "windows") { "_sheet.pyd" } else { "_sheet.so" };
    std::fs::copy(&so, pkg_dst.join(置く名)).expect("置けない");

    // **先に名乗らせる。** どの Python で、どの _sheet を読んだか。
    // status() は子の出力を素通しにするので、CI で落ちたときも
    // この2行(と import の失敗の理由)がそのままログに出る
    let 名乗り = Command::new(py)
        .args([
            "-c",
            "import sys\n\
             print('python', sys.version.split()[0], sys.executable)\n\
             import officework._sheet as e\n\
             print('_sheet:', e.__file__)\n",
        ])
        .current_dir(&stage)
        .status()
        .unwrap_or_else(|e| panic!("{py} を回せない: {e}"));
    assert!(名乗り.success(), "officework を import できない(理由は直前の出力)");

    // xlsx(sheet)と docx(doc)。**同じ .so で両方**が動くことまで見る —
    // 1つの wheel に同居させているので、片方だけ通っても足りない。
    // test_gokan.py は互換層の適合検査 — openpyxl / python-docx が居る環境では
    // 本家と結果を突き合わせ、居なければその節を飛ばしたと言って通る。
    // test_shiyou.py は**本家の受け入れ仕様から起こした検査**(本家が居なくても回る)
    for script in ["test.py", "test_doc.py", "test_gokan.py", "test_shiyou.py", "test_tex.py"] {
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(script),
            stage.join(script),
        )
        .expect("スクリプトを写せない");
        let out = Command::new(py)
            .arg(stage.join(script))
            .output()
            .unwrap_or_else(|e| panic!("{py} を回せない: {e}"));
        assert!(
            out.status.success(),
            "{script} が失敗:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
