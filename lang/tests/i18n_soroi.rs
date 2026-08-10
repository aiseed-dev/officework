//! **文言の門番を `cargo test` から回す。**
//!
//! `ui/gen_i18n.py` は前からあったが、**誰も走らせていなかった**。
//! 2026-08-10 に見たら未訳 173 句・死んだ訳 18 句まで溜まっており、
//! しかも門番自身が writer の3部屋を見ていなかったので「使われていない訳」
//! を 135 句も過大に報告し、**生きている訳を消せと言っていた**。
//!
//! 走らない検査は無いのと同じで、これは同じ日に見つけた「キーの嘘」
//! (束縛はあるのに受け口が無い)と同じ型の穴。だから Python の道具では
//! なく**試験**として置く。`cargo test --workspace` で毎回落ちる。
//!
//! 見るのは3つ:
//!
//! 1. アプリの `t!`/`tf!` の鍵が en の表に**全部ある**
//! 2. en の表に**使われていない訳が無い**
//! 3. 13言語の表が**同じ鍵の集合**を持つ(揃った言語だけを名乗る家訓)
//!
//! 鍵の取り出し方は gen_i18n.py と同じ字句走査 — 正規表現だと複数行の
//! リテラルを取り落とす(向こうも1敗して字句走査に直してある)。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace の根").to_path_buf()
}

/// `"…"` のリテラルを1つ読む。`\` の次の1文字は中身として飛ばす
/// (`\"` で終わりにしない)。返すのは**中身**(囲みの `"` は除く)
fn literal_at(s: &[u8], i: usize) -> Option<(usize, String)> {
    let mut j = i + 1;
    while j < s.len() {
        match s[j] {
            b'\\' => j += 2,
            b'"' => return Some((j + 1, String::from_utf8_lossy(&s[i + 1..j]).into_owned())),
            _ => j += 1,
        }
    }
    None
}

/// ソース1つから `ui::t!("…")` / `ui::tf!("…", …)` の鍵を集める。
/// **試験モジュールから先は見ない** — ただし `#[cfg(test)] mod tests;` の
/// **宣言**は本文の頭に来る(calc の部屋割り)ので、そこでは切らない
fn keys_in(src: &str) -> Vec<String> {
    let mut src = src;
    let mut cut_at = 0;
    while let Some(rel) = src[cut_at..].find("#[cfg(test)]") {
        let cut = cut_at + rel;
        // **文字の境で切る。** バイト数で切ると日本語の途中で割れて落ちる
        let end = src[cut..].char_indices().map(|(i, _)| cut + i).nth(64).unwrap_or(src.len());
        let next = src[cut..end].lines().nth(1).unwrap_or("").trim();
        if next.starts_with("mod ") && next.ends_with(';') {
            cut_at = cut + 1;
            continue;
        }
        src = &src[..cut];
        break;
    }
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    // `ui::item!("…")` は一覧の項の鍵(訳すのは見出しだけ)。t!/tf! と同じ鍵。
    // **ui/gen_i18n.py の走査と揃えること** — 片方だけ知っていると、生きている
    // 訳を「使われていない」と数えて消せと言い出す(2026-08-10 の一敗)
    while let Some(rel) = src[i..].find("ui::") {
        let at = i + rel;
        let rest = &src[at..];
        let head = if rest.starts_with("ui::tf!(") {
            8
        } else if rest.starts_with("ui::t!(") {
            7
        } else if rest.starts_with("ui::item!(") {
            10
        } else {
            i = at + 4;
            continue;
        };
        let mut j = at + head;
        while j < b.len() && (b[j] as char).is_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b'"' {
            if let Some((end, lit)) = literal_at(b, j) {
                out.push(lit);
                i = end;
                continue;
            }
        }
        i = at + head;
    }
    out
}

/// アプリの部屋を全部見る。**部屋が増えたら足す、ではなく毎回舐める** —
/// writer の cmds.rs・io.rs・view.rs が名指し漏れしていて、187 句が
/// 門番の目の外にあった(2026-08-10)
fn app_keys() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for dir in ["calc/src", "writer/src", "ui/src"] {
        let d = root().join(dir);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap_or_else(|e| panic!("{} が読めません: {e}", d.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .filter(|p| p.file_name().is_some_and(|n| n != "tests.rs"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "{} に .rs がありません", d.display());
        for f in files {
            let src = std::fs::read_to_string(&f).expect("読める");
            out.extend(keys_in(&src));
        }
    }
    assert!(out.len() > 500, "鍵の取り出しが壊れています(いま {} 句)", out.len());
    out
}

/// 表の鍵。**Rust の文字列としての中身**ではなく、ソースに書かれた
/// エスケープ済みの姿で比べる — アプリ側も同じ姿で取っているので揃う
fn table_keys(lang: &str) -> BTreeSet<String> {
    let p = root().join(format!("lang/src/i18n_{}.rs", lang.replace('-', "_")));
    let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let b = src.as_bytes();
    let start = src.find("= &[").expect("表の始まり") + 4;
    let mut out = BTreeSet::new();
    let mut i = start;
    while let Some(rel) = src[i..].find("(\"") {
        let at = i + rel;
        let Some((k_end, key)) = literal_at(b, at + 1) else { break };
        // 鍵の次は訳のリテラル。読み飛ばして次の項目へ
        let Some(vq) = src[k_end..].find('"').map(|r| k_end + r) else { break };
        let Some((v_end, _)) = literal_at(b, vq) else { break };
        out.insert(key);
        i = v_end;
    }
    out
}

#[test]
fn 英語の表に未訳が無い() {
    let app = app_keys();
    let en = table_keys("en");
    let missing: Vec<&String> = app.difference(&en).collect();
    assert!(
        missing.is_empty(),
        "未訳が {} 句あります(python3 ui/gen_i18n.py --missing で骨組みが出ます):\n{}",
        missing.len(),
        missing.iter().take(10).map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn 使われていない訳が残っていない() {
    let app = app_keys();
    let en = table_keys("en");
    let dead: Vec<&String> = en.difference(&app).collect();
    assert!(
        dead.is_empty(),
        "もう使っていない訳が {} 句あります(tools/i18n_edit.py --drop-dead で外せます):\n{}",
        dead.len(),
        dead.iter().take(10).map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n")
    );
}

/// **揃っていない言語を名乗らない。** 1つでも欠けた言語があるなら、
/// その言語は「対応しています」と言ってよい状態ではない
#[test]
fn 十三言語の表が同じ鍵を持つ() {
    let en = table_keys("en");
    for lang in lang::i18n_tables::LANGS {
        if *lang == "en" {
            continue;
        }
        let t = table_keys(lang);
        let missing = en.difference(&t).count();
        let extra = t.difference(&en).count();
        assert_eq!(
            (missing, extra),
            (0, 0),
            "{lang} の表が en と揃っていません(足りない {missing} 句 / 余分 {extra} 句)。\
             ui/i18n/{lang}.json に訳を書いて python3 ui/gen_lang.py {lang}"
        );
    }
}

/// 穴埋めの数が言語で食い違うと、**実行時に穴が埋まらないか panic する**。
/// 表を作るときにも見ているが、手で直せてしまうのでここでも見る
#[test]
fn 穴埋めの数が言語をまたいで揃う() {
    let holes = |s: &str| s.match_indices("{}").count();
    let en = table_keys("en");
    for lang in lang::i18n_tables::LANGS {
        let t = lang::i18n_tables::table(lang).expect("登録済み");
        for (k, v) in t {
            if !en.contains(*k) {
                continue;
            }
            assert_eq!(
                holes(k),
                holes(v),
                "{lang}: 穴の数が違います\n  鍵 {k}\n  訳 {v}"
            );
        }
    }
}
