//! **紙面の回帰検査。** 同じ入力から同じ絵が出るかを見ます。
//!
//! 2026-08-29 の依頼(SEKKEI「チャートと vello の受け持ち」)の
//! 使い道の1つです。
//!
//! # なぜ絵で見るのか
//!
//! 組版の直しは、**数の試験が全部緑のまま見た目だけ壊れる**ことがあります。
//! 実際にこの1週間で3度ありました。
//!
//! * セルの塗りが紙に出ていなかった(模型には在ったのに組む所が見ていない)
//! * 太字が本文から消えていた(run をまとめる条件が大きさしか見ていない)
//! * 紙の色を敷いておらず、字が透明の上に乗って見えなかった
//!
//! どれも「開いて見る」まで分かりませんでした。ここは**開いて見る係を
//! 機械にやらせる**物です。
//!
//! # 控えの持ち方
//!
//! 絵そのものは置きません(バイト数が大きく、書体の版で変わります)。
//! **指紋**([`paper::e::E::yubi`])を `paper/tests/kaiki.txt` に控え、
//! 次から突き合わせます。
//!
//! 紙面1枚につき指紋を2つ取ります。
//!
//! * `罫線` — 書体を渡さずに描いた絵です。線・塗り・紙の色だけが写ります
//! * `字` — 書体を渡して描いた絵です
//!
//! 分けたのは、CI の機械と手元とで入っている書体が違うからです。1つに
//! まとめると、CI では毎回違う指紋が出て、この検査を切るしかなくなります。
//!
//! `字` は、控えを取ったときと**同じ書体のときだけ**突き合わせます。
//! `罫線` は紙面によります。
//!
//! * 表計算は、列の幅も行の高さもシートが持っているので、書体が変わっても
//!   線と塗りの位置は動きません。**いつも突き合わせます**
//! * 文書は、行の折り返しが字幅で決まります。書体が変わると表の高さも
//!   改ページの位置も動くので、`字` と同じに扱います
//!
//! はじめの3件のうち2件(塗りと紙の色)は、表計算の罫線の側で捕まります。
//!
//! 違いが出たら、この試験は**絵を書き出して**から落ちます。
//! `target/kaiki/` に出るので、目で見比べてください。
//!
//! # 絵を目で見たいとき
//!
//! ```text
//! KAIKI=みる cargo test -p paper --features e --test kaiki -- --nocapture
//! ```
//!
//! 突き合わせずに `target/kaiki/` へ書き出します。
//!
//! # 控えを取り直すとき
//!
//! 見た目を**わざと**変えたときは、控えを取り直します。
//!
//! ```text
//! KAIKI=とりなおす cargo test -p paper --features e --test kaiki
//! ```
//!
//! **落ちたからといって、まず取り直さないでください。** 取り直すのは、
//! 出た絵を見て「こちらが正しい」と決めた後です。

#![cfg(feature = "e")]

use std::collections::BTreeMap;
use std::path::PathBuf;

/// 控えの置き場
fn hikae_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/kaiki.txt")
}

/// 出た絵の置き場(落ちたときだけ書きます)
fn dashi_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/kaiki")
}

fn hikae() -> BTreeMap<String, String> {
    std::fs::read_to_string(hikae_path())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(char::is_whitespace))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// 見本の紙面を作る。**実物の道を通します** — 紙にするのと同じ組み方です
/// 返す組は(名前, 紙面, **書体で形が変わるか**)です。3つ目が真なら、
/// 控えと違う書体のときは罫線も突き合わせません
fn mihon() -> Vec<(&'static str, paper::pdfw::Leaf, bool)> {
    let mut out = Vec::new();

    // ① 表計算。罫線・帯・縞・行番号・ヘッダーとフッター
    let mut sh = book::Sheet::new("売上");
    sh.print_gridlines = true;
    sh.print_headings = true;
    sh.header = Some("&C四月の売上".into());
    sh.footer = Some("&C&P / &N".into());
    for (c, t) in ["支店", "4月", "5月", "合計"].iter().enumerate() {
        let mut cell = book::Cell {
            value: book::Value::Text((*t).into()),
            ..Default::default()
        };
        cell.fmt.bold = true;
        cell.fmt.fill = Some("DDE7F0".into());
        cell.fmt.borders.bottom.on = true;
        sh.set(book::Pos::new(0, c as u32), cell);
    }
    for (r, m) in ["東京", "大阪", "名古屋"].iter().enumerate() {
        let r = r as u32 + 1;
        sh.set(
            book::Pos::new(r, 0),
            book::Cell { value: book::Value::Text((*m).into()), ..Default::default() },
        );
        for c in 1..3u32 {
            let mut cell = book::Cell {
                value: book::Value::Number((r * 300 + c * 120) as f64),
                ..Default::default()
            };
            cell.fmt.number_format = Some("#,##0".into());
            if r % 2 == 0 {
                cell.fmt.fill = Some("F5F7FA".into());
            }
            sh.set(book::Pos::new(r, c), cell);
        }
        let mut sum = book::Cell::input(&format!("=SUM(B{0}:C{0})", r + 1));
        sum.fmt.number_format = Some("#,##0".into());
        sh.set(book::Pos::new(r, 3), sum);
    }
    let mut bk = book::Book::new();
    bk.sheets.clear();
    bk.sheets.push(sh);
    book::calc::recalc_all(&mut bk);
    let setup = paper::grid::PrintSetup::default();
    if let Ok(leaf) = paper::grid::sheet_leaf(&bk.sheets[0], paper::Paper::default(), &setup) {
        // 列の幅も行の高さもシートが持つので、書体では動きません
        out.push(("表計算", leaf, false));
    }

    // ② 文書。見出し・本文・表・註記の帯
    let adoc = "\
= 四月の売上報告

== 明細

|===
|品名 |数量 |金額

|ボールペン |12 |1,800
|ノート |24 |2,880
|===

NOTE: 単価は税抜きです。

本文は *太字* と普通の字が混ざります。日本語の行組みは JIS X 4051 の
禁則で折ります。行頭に「。」や「、」が来ないよう追い出します。
";
    if let Ok(doc) = kumihan::adoc::parse(adoc) {
        if let Ok((sheet, page, _bytes)) = paper::doc_to_sheet(&doc, None) {
            if let Some(leaf) = paper::doc_leaf(&sheet, page, 0) {
                // 行の折り返しが字幅で決まるので、書体で形が動きます
                out.push(("文書", leaf, true));
            }
        }
    }
    out
}

/// 書体の見分け。**同じ書体か**を言えればよいので、名前とバイト列の指紋です
fn shotai_shirushi(fam: &str, data: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{}/{h:016x}", fam.replace(char::is_whitespace, "_"))
}

/// **同じ入力から同じ絵が出る。**
#[test]
fn the_same_paper_draws_the_same_picture() {
    let sasi = std::env::var("KAIKI").unwrap_or_default();
    let torinaosu = sasi == "とりなおす";
    // **目で見たいとき。** 突き合わせずに絵だけ書き出します
    let miru = sasi == "みる";
    let (fam, _) = kumihan::font::for_document(None).expect("日本語の書体が要る");
    let data = kumihan::font::load(fam).expect("書体が読めない");
    let shirushi = shotai_shirushi(&fam.name, &data);

    let mae = hikae();
    // 控えを取ったときと同じ書体か。違えば**字の指紋は見送ります**
    let onaji_shotai = mae.get("書体").is_some_and(|m| *m == shirushi);
    if !onaji_shotai && !torinaosu {
        eprintln!(
            "控えは別の書体で取られています。\n\
             書体で形が動かない紙面の罫線だけ突き合わせます\n\
             \u{3000}控え {}\n\u{3000}いま {shirushi}",
            mae.get("書体").map(String::as_str).unwrap_or("(なし)")
        );
    }

    let mut ima: BTreeMap<String, String> = BTreeMap::new();
    ima.insert("書体".into(), shirushi);
    let mut chigau: Vec<String> = Vec::new();
    let mut mita = 0usize;

    for (na, leaf, ugoku) in mihon() {
        let (w, h) = leaf.size_mm.unwrap_or((210.0, 297.0));
        for (shu, e) in [
            ("罫線", paper::e::egaku(&leaf, w, h, 3.0)),
            ("字", paper::e::egaku_with(&leaf, w, h, 3.0, Some(&data))),
        ] {
            let kagi = format!("{na}/{shu}");
            let yubi = e.yubi();
            ima.insert(kagi.clone(), yubi.clone());
            if miru {
                let dir = dashi_dir();
                let _ = std::fs::create_dir_all(&dir);
                if let Ok(png) = e.png() {
                    let michi = dir.join(format!("{na}_{shu}.png"));
                    let _ = std::fs::write(&michi, png);
                    eprintln!("{} {}×{} 指紋 {yubi}", michi.display(), e.w, e.h);
                }
                continue;
            }
            if (shu == "字" || ugoku) && !onaji_shotai {
                continue;
            }
            mita += 1;
            match mae.get(&kagi) {
                Some(m) if *m == yubi => {}
                Some(m) => {
                    // **落ちる前に絵を出します。** 目で見比べられないと直せません
                    let dir = dashi_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    if let Ok(png) = e.png() {
                        let _ = std::fs::write(dir.join(format!("{na}_{shu}.png")), png);
                    }
                    chigau.push(format!("  {kagi}: 控え {m} → いま {yubi}"));
                }
                None => chigau.push(format!("  {kagi}: 控えがありません(いま {yubi})")),
            }
        }
    }
    assert!(ima.len() > 1, "見本の紙面が1枚も組めていない");
    assert!(torinaosu || miru || mita > 0, "突き合わせた指紋が1つも無い");
    if miru {
        return;
    }

    if torinaosu {
        let mut s = String::from(
            "# 紙面の指紋(paper/tests/kaiki.rs が使います)\n\
             #\n\
             # **手で書き替えないでください。** 取り直すときは:\n\
             #   KAIKI=とりなおす cargo test -p paper --features e --test kaiki\n\
             #\n\
             # 書体の版が変わると字の形が変わり、指紋も変わります。\n\
             # そのときは出た絵を見て、正しいことを確かめてから取り直します。\n\
             #\n\
             # 「罫線」は書体を渡さずに描いた絵、「字」は渡して描いた絵です。\n\
             # 「書体」の行と今の書体が違うとき、「字」の行は見送られます。\n",
        );
        for (k, v) in &ima {
            s.push_str(&format!("{k} {v}\n"));
        }
        std::fs::write(hikae_path(), s).expect("控えが書けない");
        return;
    }

    assert!(
        chigau.is_empty(),
        "紙面の絵が控えと違います:\n{}\n\n\
         出た絵は target/kaiki/ にあります。**目で見比べてください。**\n\
         こちらが正しいと決めたら、控えを取り直します:\n\
         \u{3000}KAIKI=とりなおす cargo test -p paper --features e --test kaiki",
        chigau.join("\n")
    );
}
