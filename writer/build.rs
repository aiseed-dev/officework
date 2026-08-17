//! **Windows の .exe に絵を焼く。** これが無いと、エクスプローラでも
//! タスクバーでも無地の四角になる(2026-08-17 のアルファの棚卸し)。
//!
//! 絵の正本は `packaging/icons/officework-writer.svg` の1枚で、`.ico` は
//! `tools/make_icons.py` が起こしてコミットしてある。
//!
//! **Windows の的のときだけ**動く。他の的では何もしないので、Linux と
//! macOS の組み立てには一切触らない。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("../packaging/icons/officework-writer.ico");
        // 失敗しても組み立ては止めない — 絵が無いだけで、動く物は出来る
        if let Err(e) = r.compile() {
            println!("cargo:warning=絵を焼けませんでした: {e}");
        }
    }
    println!("cargo:rerun-if-changed=../packaging/icons/officework-writer.ico");
}
