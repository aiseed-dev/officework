// mac で extension-module の cdylib を組むための指定。
//
// extension-module は libpython に繋がないので、mac のリンカには
// 「未解決の記号は読み込み時に解決してよい」(-undefined dynamic_lookup)を
// 渡す必要がある。maturin は wheel を組むときにこれを自動で足すが、
// 素の cargo build は足さない — python_smoke の中の組み立てが mac だけ
// リンクで落ちたのはこれ(2026-08-24 の CI)。pyo3 の手引きどおり、
// この1行で cdylib のリンクにだけ効く(Linux と Windows では何もしない)。
fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
