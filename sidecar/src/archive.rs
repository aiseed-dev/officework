//! **ZIP の配管。** 向こうの TypeScript が xlsx を組み立てる道。
//!
//! **こちらは中身を解釈しない** — 部品を数え、取り出し、探し、当てて書くだけ。
//! xlsx の意味は `value` の組が持つ。

use std::collections::BTreeMap;
use std::io::{Read, Write};

use serde_json::{Value, json};

//
// 向こうの TypeScript が xlsx を組み立てる道。**こちらは中身を解釈しない** —
// 部品を数え、取り出し、探し、当てて書くだけ。xlsx の意味は上の組が持つ。

/// 要求の配列から名前を取り出す。`null` や欠けは空として扱う。
pub(crate) fn names(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// 部品ひとつの札。**向こうの `archiveEntrySchema` と同じ4欄。**
pub(crate) fn entry_value(f: &zip::read::ZipFile<'_>) -> Value {
    json!({
        "name": f.name(),
        "crc32": f.crc32(),
        "compressedSize": f.compressed_size(),
        "uncompressedSize": f.size(),
    })
}

pub(crate) fn open_zip(path: &str) -> Result<zip::ZipArchive<std::io::BufReader<std::fs::File>>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{path}: 開けません: {e}"))?;
    zip::ZipArchive::new(std::io::BufReader::new(f))
        .map_err(|e| format!("{path}: ZIP として読めません: {e}"))
}

/// `archive_manifest` — 部品の一覧。**原本の並びのまま返す。**
pub(crate) fn archive_manifest(path: &str) -> Result<Vec<Value>, String> {
    let mut z = open_zip(path)?;
    (0..z.len())
        .map(|i| z.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{path}: {e}")))
        .collect()
}

/// `read_entries` — 名前で指した部品を `output_dir` へ出し、置いた径路を返す。
///
/// **名前をそのまま径路にしない。** `xl/worksheets/sheet1.xml` の `/` で
/// 掘るのは呼ぶ側の想定ではないし、`..` を含む名前(zip slip)を渡されたら
/// 出力先の外へ書いてしまう。**平らな名前に潰して置く。**
pub(crate) fn read_entries(path: &str, want: &[String], output_dir: &str) -> Result<Vec<Value>, String> {
    let mut z = open_zip(path)?;
    std::fs::create_dir_all(output_dir).map_err(|e| format!("{output_dir}: 作れません: {e}"))?;
    let mut out = Vec::new();
    for (i, name) in want.iter().enumerate() {
        let mut f = match z.by_name(name) {
            Ok(f) => f,
            // **無い部品は黙って飛ばす。** 向こうは「あれば読む」で呼び、
            // 答えの数が減ったことで無かったと分かる作り
            Err(_) => continue,
        };
        let flat = format!("{i}-{}", name.replace(['/', '\\'], "_"));
        let dest = std::path::Path::new(output_dir).join(&flat);
        let mut w = std::fs::File::create(&dest)
            .map_err(|e| format!("{}: 作れません: {e}", dest.display()))?;
        std::io::copy(&mut f, &mut w).map_err(|e| format!("{name}: 出せません: {e}"))?;
        out.push(json!({ "name": name, "path": dest.display().to_string() }));
    }
    Ok(out)
}

/// `scan_entries` — 名前で指した部品に文字列が入っているか。
///
/// **解いた中身をそのまま見る。** UTF-8 として読めない部品もあるので、
/// 字ではなくバイトの並びで探す(xlsx の XML は UTF-8 なので同じことだが、
/// 画像などを渡されても落ちない)。
pub(crate) fn scan_entries(path: &str, want: &[String], needle: &str) -> Result<Vec<String>, String> {
    let mut z = open_zip(path)?;
    let pat = needle.as_bytes();
    let mut hit = Vec::new();
    for name in want {
        let Ok(mut f) = z.by_name(name) else { continue };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("{name}: 読めません: {e}"))?;
        if pat.is_empty() || buf.windows(pat.len()).any(|w| w == pat) {
            hit.push(name.clone());
        }
    }
    Ok(hit)
}

/// `save_archive` — 原本に差し替え・削除・追加を当てて、別名で書く。
///
/// **急所は「触っていない部品を解かずに写す」こと。** 向こうの TypeScript は
/// 保存のあと `assertManifestPreserved` で、触っていない部品の **crc32 と
/// 圧縮後の大きさ**が変わっていないことを確かめる。解いて詰め直すと、
/// deflate の水準が少し違うだけで圧縮後の大きさが変わり、**向こうが
/// 「部品が変わった」と言って保存を止める**。
///
/// `raw_copy_file` は圧縮済みの流れをそのまま写す。だから原本の部品は
/// **1バイトも変わらない** — 「触っていない所を壊さない」を字面どおりに守る。
pub(crate) fn save_archive(req: &Value) -> Result<Value, String> {
    let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let pairs = |k: &str| -> Vec<(String, String)> {
        req.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        Some((
                            x.get("name")?.as_str()?.to_string(),
                            x.get("contentPath")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let (source, target) = (s("sourcePath"), s("targetPath"));
    let replacements: BTreeMap<String, String> = pairs("replacements").into_iter().collect();
    let additions = pairs("additions");
    let removals: std::collections::BTreeSet<String> =
        names(req.get("removals")).into_iter().collect();

    let mut z = open_zip(&source)?;
    let before: Vec<Value> = (0..z.len())
        .map(|i| z.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{source}: {e}")))
        .collect::<Result<_, _>>()?;

    let out = std::fs::File::create(&target).map_err(|e| format!("{target}: 作れません: {e}"))?;
    let mut w = zip::ZipWriter::new(std::io::BufWriter::new(out));
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 原本の並びを保ったまま写す。差し替えはその場で、削除は飛ばす
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| format!("{source}: {e}"))?;
        let name = f.name().to_string();
        if removals.contains(&name) {
            continue;
        }
        match replacements.get(&name) {
            Some(src) => {
                drop(f);
                let body = std::fs::read(src).map_err(|e| format!("{src}: 読めません: {e}"))?;
                w.start_file(&name, opts).map_err(|e| format!("{name}: 書けません: {e}"))?;
                w.write_all(&body).map_err(|e| format!("{name}: 書けません: {e}"))?;
            }
            // **解かずに写す。** ここを `copy` にすると向こうの検査で落ちる
            None => w.raw_copy_file(f).map_err(|e| format!("{name}: 写せません: {e}"))?,
        }
    }
    // 追加は末尾へ。**原本に同じ名前があれば差し替えで済んでいる**ので、
    // ここで二重に書くと ZIP に同名の部品が2つ並ぶ
    let had: std::collections::BTreeSet<String> =
        before.iter().filter_map(|e| e.get("name")?.as_str().map(str::to_string)).collect();
    for (name, src) in &additions {
        if had.contains(name) {
            continue;
        }
        let body = std::fs::read(src).map_err(|e| format!("{src}: 読めません: {e}"))?;
        w.start_file(name, opts).map_err(|e| format!("{name}: 書けません: {e}"))?;
        w.write_all(&body).map_err(|e| format!("{name}: 書けません: {e}"))?;
    }
    w.finish().map_err(|e| format!("{target}: 閉じられません: {e}"))?;

    let mut z2 = open_zip(&target)?;
    let after: Vec<Value> = (0..z2.len())
        .map(|i| z2.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{target}: {e}")))
        .collect::<Result<_, _>>()?;
    Ok(json!({ "beforeEntries": before, "afterEntries": after }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 部品を3つ持つ ZIP を作る。
    ///
    /// **わざと違う圧縮の水準で詰める。** 最初はここを既定のままにしていて、
    /// 中の実装を「解いて詰め直す」形に替えても試験が**通ってしまった** —
    /// 同じライブラリの同じ水準で詰め直せば同じ大きさになるのは当たり前で、
    /// 何も見ていなかった。実物は Excel や JSZip が詰めた物で、水準は
    /// こちらと違う。**型紙を本番に似せないと、検査は空を打つ。**
    fn make_zip(path: &std::path::Path) {
        let f = std::fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(2));
        // **繰り返しだけの中身では水準の差が出ない**(どの水準でも同じ所まで
        // 縮む)。実物の worksheet に似せて、変化のある中身にする
        let xmlish = |tag: &str| -> String {
            (0..400)
                .map(|i| format!(r#"<{tag} r="A{i}" s="{}"><v>{}</v></{tag}>"#, i % 7, i * 37 % 1009))
                .collect()
        };
        for (name, body) in [
            ("[Content_Types].xml", xmlish("Override")),
            ("xl/workbook.xml", xmlish("sheet")),
            ("xl/worksheets/sheet1.xml", format!("{}うたかたの泡", xmlish("c"))),
        ] {
            w.start_file(name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn manifest(path: &std::path::Path) -> Vec<(String, u32, u64)> {
        let mut z = open_zip(&path.display().to_string()).unwrap();
        (0..z.len())
            .map(|i| {
                let f = z.by_index(i).unwrap();
                (f.name().to_string(), f.crc32(), f.compressed_size())
            })
            .collect()
    }
    /// **触っていない部品は、解かずにそのまま写す。**
    ///
    /// 向こうの `assertManifestPreserved` は crc32 だけでなく**圧縮後の
    /// 大きさ**も見る。解いて詰め直すと deflate の水準の差で後者が動き、
    /// 保存が「部品が変わった」と止められる。`raw_copy_file` を
    /// `std::io::copy` に替えたらこの試験が落ちる。
    #[test]
    fn untouched_parts_keep_even_their_compressed_size() {
        let dir = std::env::temp_dir().join("ow-plumb-1");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, dst, body) = (dir.join("a.xlsx"), dir.join("b.xlsx"), dir.join("new.xml"));
        make_zip(&src);
        std::fs::write(&body, "<workbook/>").unwrap();

        let before = manifest(&src);
        let req = json!({
            "sourcePath": src.display().to_string(),
            "targetPath": dst.display().to_string(),
            "replacements": [{"name": "xl/workbook.xml", "contentPath": body.display().to_string()}],
            "removals": [],
            "additions": [],
        });
        let r = save_archive(&req).expect("保存できない");
        let after = manifest(&dst);

        assert_eq!(before.len(), after.len(), "部品の数が変わった");
        assert_eq!(
            before.iter().map(|e| &e.0).collect::<Vec<_>>(),
            after.iter().map(|e| &e.0).collect::<Vec<_>>(),
            "**並びが変わった** — 原本の順を保つこと"
        );
        for (b, a) in before.iter().zip(&after) {
            if b.0 == "xl/workbook.xml" {
                assert_ne!(b.1, a.1, "差し替えたのに中身が同じ");
                continue;
            }
            assert_eq!((b.1, b.2), (a.1, a.2), "{}: 触っていないのに変わった", b.0);
        }
        // 答えの形も向こうの schema どおりか
        assert!(r["beforeEntries"].is_array() && r["afterEntries"].is_array());
        assert_eq!(r["beforeEntries"][0]["name"], "[Content_Types].xml");
    }

    /// 削除と追加。**追加は原本に無い名前だけ** — 同じ名前を二重に書くと
    /// ZIP の中に同名の部品が2つ並び、読み手によって答えが変わる
    #[test]
    fn delete_and_add_work_without_duplicate_names() {
        let dir = std::env::temp_dir().join("ow-plumb-2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, dst, body) = (dir.join("a.xlsx"), dir.join("b.xlsx"), dir.join("x.xml"));
        make_zip(&src);
        std::fs::write(&body, "<x/>").unwrap();

        let req = json!({
            "sourcePath": src.display().to_string(),
            "targetPath": dst.display().to_string(),
            // **原本にある名前を「追加」で渡す。** 差し替えとして扱われるべき
            "replacements": [{"name": "xl/workbook.xml", "contentPath": body.display().to_string()}],
            "removals": ["xl/worksheets/sheet1.xml"],
            "additions": [
                {"name": "xl/new.xml", "contentPath": body.display().to_string()},
                {"name": "xl/workbook.xml", "contentPath": body.display().to_string()},
            ],
        });
        save_archive(&req).expect("保存できない");
        let names: Vec<String> = manifest(&dst).into_iter().map(|e| e.0).collect();
        assert!(!names.contains(&"xl/worksheets/sheet1.xml".to_string()), "消えていない");
        assert!(names.contains(&"xl/new.xml".to_string()), "足されていない");
        assert_eq!(
            names.iter().filter(|n| *n == "xl/workbook.xml").count(),
            1,
            "**同じ名前が2つ並んだ** — 原本にある名前の追加は差し替えで済んでいる"
        );
    }

    /// `read_entries` は**平らな名前で置く**。`..` を含む名前を渡されても
    /// 出力先の外へ書かない(zip slip)
    #[test]
    fn extraction_stays_inside_the_output_dir() {
        let dir = std::env::temp_dir().join("ow-plumb-3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, out) = (dir.join("a.xlsx"), dir.join("out"));
        make_zip(&src);
        let got = read_entries(
            &src.display().to_string(),
            &["xl/worksheets/sheet1.xml".into(), "無い部品.xml".into()],
            &out.display().to_string(),
        )
        .expect("取り出せない");
        assert_eq!(got.len(), 1, "**無い部品で落ちない・数で分かる**");
        let p = std::path::PathBuf::from(got[0]["path"].as_str().unwrap());
        assert_eq!(p.parent().unwrap(), out, "出力先の外に置いた");
        assert!(std::fs::read_to_string(&p).unwrap().contains("うたかたの泡"), "中身が違う");
    }

    #[test]
    fn search_looks_inside_the_contents() {
        let dir = std::env::temp_dir().join("ow-plumb-4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.xlsx");
        make_zip(&src);
        let all: Vec<String> =
            ["[Content_Types].xml".into(), "xl/workbook.xml".into()].into_iter().collect();
        let s = src.display().to_string();
        assert_eq!(scan_entries(&s, &all, "<sheet r=\"A3\"").unwrap(), vec!["xl/workbook.xml".to_string()]);
        assert!(scan_entries(&s, &all, "無い字").unwrap().is_empty(), "無い字が見つかった");
    }
}
