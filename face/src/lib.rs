//! **持ち運べる層** — アプリに何があるかを、描き方から切り離して持つ。
//!
//! リボンの表(タブ・組・ボタンの並び)、14言語の札、キーの割り当て、
//! 関数名の14言語の表、settings.toml の読み書き、窓の位置の控え。
//!
//! # なぜ別のクレートなのか
//!
//! 発注者 2026-08-15「**GPUI の殻を kotlin と swift で書く**」。殻が増える
//! なら、殻でない物が殻の外に出ていないと、同じ表と同じ命令を殻の数だけ
//! 書き直すことになり、必ずずれる。
//!
//! **ここに gpui が入っていないことは cargo が保証する。** `Cargo.toml` の
//! 依存が空なのがその壁で、気をつけて守る物ではない(SEKKEI「クレート境界は
//! Rust で唯一の本物の壁」)。
//!
//! # ui との分かれ目
//!
//! - `face` = **何があるか**(表・札・割り当て・設定)
//! - `ui` = **どう描くか**(gpui の部品・絵・束縛の組み立て)
//!
//! `ui` はここを丸ごと再公開しているので、呼ぶ側は今までどおり
//! `ui::ribbon` や `ui::settings` で届く。移してもアプリ側は1行も
//! 変わっていない。
//!
//! # 移していない物(境目の記録)
//!
//! - `bindings_for` — gpui の `KeyBinding` を組み立てるので `ui` に残る。
//!   芯の [`compose_keys`] だけがこちらに来ていて、**鍵が読めるかどうかの
//!   判定は引数で受け取る**(gpui の殻は `gpui::Keystroke::parse` を渡し、
//!   Kotlin / Swift の殻はそれぞれの読み手を渡す)
//! - `icons` の SVG を絵にする所 — `ui::svg_to_png`(resvg)は `ui` に残る。
//!   こちらが持つのは**SVG の字面**だけで、これは持ち運べる

pub mod combo;
/// フォルダの中身を並べる(名前で種類が決まる)
pub mod folder;
pub mod funcs;
// gen_funcs:begin(この間は calc/gen_funcs.py が生成する — 手で書かない)
pub mod funcs_de;
pub mod funcs_en;
pub mod funcs_es;
pub mod funcs_fr;
pub mod funcs_id;
pub mod funcs_it;
pub mod funcs_ko;
pub mod funcs_pt;
pub mod funcs_pt_br;
pub mod funcs_ru;
pub mod funcs_tr;
pub mod funcs_vi;
pub mod funcs_zh;
pub mod funcs_zh_tw;
pub mod funcs_tables;
// gen_funcs:end
pub mod icons;
pub mod keys;
pub mod ribbon;
pub mod search;

// gen_lang:begin(この間は ui/gen_lang.py が生成する — 手で書かない)
pub mod ribbon_de;
pub mod ribbon_en;
pub mod ribbon_es;
pub mod ribbon_fr;
pub mod ribbon_id;
pub mod ribbon_it;
pub mod ribbon_ko;
pub mod ribbon_pt;
pub mod ribbon_pt_br;
pub mod ribbon_ru;
pub mod ribbon_tr;
pub mod ribbon_vi;
pub mod ribbon_zh;
pub mod ribbon_zh_tw;
// gen_lang:end

pub mod ribbon_tables;
pub mod settings;
pub mod winstate;

pub use keys::{compose_keys, default_keys, KeyWarn, KEYS_CALC, KEYS_COMMON, KEYS_WRITER};

#[cfg(test)]
mod kabe {
    /// **この層に描画の物を入れない**、を試験で見張る(2026-08-15)。
    ///
    /// face の値打ちは「gpui を持たないこと」そのもので、依存を1行足せば
    /// 黙って消える。**気をつけて守る物ではなく、落ちて知らせる物にする。**
    ///
    /// 見るのは直の依存だけ。孫まで見るのは CI の `cargo tree` の役
    /// (GPL の3クレート ztracing / zlog / ztracing_macro も gpui 経由で
    /// しか来ないので、gpui を止めれば一緒に止まる)。
    #[test]
    fn face_に描画のクレートを入れない() {
        let toml = include_str!("../Cargo.toml");
        // 註や説明の行は見ない — 依存として書かれた行だけを見る
        for line in toml.lines() {
            let l = line.trim();
            if l.starts_with('#') || !l.contains('=') {
                continue;
            }
            let name = l.split(['=', ' ']).next().unwrap_or("");
            assert!(
                !matches!(name, "gpui" | "resvg" | "tiny-skia" | "usvg" | "winit" | "wgpu"),
                "face に描画のクレート `{name}` を入れてはいけません。\n\
                 ここは Kotlin / Swift の殻からも読む層で、gpui を引いた瞬間に\n\
                 その値打ちが消えます(発注者 2026-08-15「GPUI の殻を kotlin と\n\
                 swift で書く」)。描く物は ui / calc / writer の側へ。"
            );
        }
    }
}
