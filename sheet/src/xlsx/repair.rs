//! **壊れた xlsx から読める部品だけを拾う**(2026-08-22。台帳「開いて修復」)。
//!
//! xlsx は zip です。zip は末尾の「中央目録」に何がどこにあるかを書いてあり、
//! 普通の読み手はそこを見ます。**壊れるとまずここが読めなくなります** —
//! 転送が途中で切れた、末尾が欠けた、目録だけが化けた、のどれでも。
//!
//! ここでは目録を当てにせず、**頭から `PK\x03\x04` を探して**部品を1つずつ
//! 拾います。拾えた部品だけで zip を組み直し、**普通の読み手にそのまま
//! 渡します**。読み手は元から「無い部品は飛ばす」造りなので、拾えなかった
//! 部品はそのまま欠けた状態で開きます。
//!
//! 拾えなかった部品は[`Salvage::lost`]に名前で並べます。**「修復しました」
//! だけで済ませません**(2026-08-09 発注者確定)。

use std::io::{Cursor, Write};

/// 拾った結果。
pub struct Salvage {
    /// 組み直した zip の中身。これを [`super::read`] に渡します
    pub bytes: Vec<u8>,
    /// 拾えた部品の名前
    pub kept: Vec<String>,
    /// **拾えなかった部品**(名前, なぜ)。名前が読めなければ「(名前も読めません)」
    pub lost: Vec<(String, String)>,
}

impl Salvage {
    /// 1つでも拾えたか。
    pub fn any(&self) -> bool {
        !self.kept.is_empty()
    }
}

/// 局所ヘッダの目印。zip の各部品はこの4バイトで始まります
const LOCAL: &[u8; 4] = b"PK\x03\x04";
/// 中央目録の目印。ここから先に部品の本体はありません
const CENTRAL: &[u8; 4] = b"PK\x01\x02";

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}
fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

/// `raw` の中の `at` 以降で、`needle` が最初に出る位置。
fn find_from(raw: &[u8], at: usize, needle: &[u8; 4]) -> Option<usize> {
    if at >= raw.len() {
        return None;
    }
    raw[at..].windows(4).position(|w| w == needle).map(|i| at + i)
}

/// **壊れた xlsx から拾えるだけ拾う。**
///
/// 中央目録を当てにせず、局所ヘッダを頭から探します。
/// 大きさが書いていない部品(データ記述子つき)は、**次の目印までを本体と
/// みなして**解きます。解けなければその部品は捨て、名前を控えます。
///
/// 元の中身は一切書き換えません。返すのは新しく組んだ zip です。
pub fn salvage(raw: &[u8]) -> Salvage {
    let mut kept: Vec<String> = Vec::new();
    let mut lost: Vec<(String, String)> = Vec::new();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // 同じ名前の部品が二度出てきたときは**先に出た方を採ります**。
    // 後から書くと、途中で切れた側で上書きすることがあります
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    let mut at = 0usize;
    while let Some(pos) = find_from(raw, at, LOCAL) {
        at = pos + 4;
        // 局所ヘッダ: 4 目印 / 2 版 / 2 旗 / 2 方式 / 4 時刻 / 4 CRC
        //           / 4 圧縮後 / 4 圧縮前 / 2 名前長 / 2 追加長
        let (Some(flags), Some(method), Some(csize), Some(usize_), Some(nlen), Some(xlen)) = (
            u16le(raw, pos + 6),
            u16le(raw, pos + 8),
            u32le(raw, pos + 18),
            u32le(raw, pos + 22),
            u16le(raw, pos + 26),
            u16le(raw, pos + 28),
        ) else {
            lost.push((
                "(名前も読めません)".into(),
                "頭が途中で切れています".into(),
            ));
            break;
        };
        let name_at = pos + 30;
        let data_at = name_at + nlen as usize + xlen as usize;
        let Some(name_bytes) = raw.get(name_at..name_at + nlen as usize) else {
            lost.push(("(名前も読めません)".into(), "名前が途中で切れています".into()));
            break;
        };
        let name = String::from_utf8_lossy(name_bytes).to_string();
        if name.is_empty() || data_at > raw.len() {
            lost.push((
                if name.is_empty() { "(名前も読めません)".into() } else { name },
                "中身が途中で切れています".into(),
            ));
            continue;
        }
        // 置き場所が分かったので、次の探索はここから
        at = data_at;
        if seen.contains(&name) {
            continue;
        }
        // 大きさが書いていない(旗の3ビット目=データ記述子)ときは、
        // **次の目印までを本体とみなす**
        let end = if csize == 0 && (flags & 0x08) != 0 {
            let a = find_from(raw, data_at, LOCAL);
            let b = find_from(raw, data_at, CENTRAL);
            match (a, b) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => raw.len(),
            }
        } else {
            (data_at + csize as usize).min(raw.len())
        };
        let body = &raw[data_at..end];
        // ディレクトリの印(名前が / で終わる)は中身を持ちません
        if name.ends_with('/') {
            continue;
        }
        let plain = match method {
            0 => Ok(body.to_vec()),
            8 => resolve_it(body),
            m => Err(format!("知らない圧縮の方式です({m})")),
        };
        match plain {
            Ok(p) => {
                // 大きさが書いてあるなら突き合わせる。合わないものは**捨てます** —
                // 半端に解けた XML を渡すと、そこから先が黙って消えます
                if usize_ != 0 && p.len() as u32 != usize_ {
                    lost.push((name, "中身の大きさが合いません".into()));
                    continue;
                }
                if out.start_file(name.as_str(), opts).is_err() || out.write_all(&p).is_err() {
                    lost.push((name, "組み直せません".into()));
                    continue;
                }
                seen.insert(name.clone());
                kept.push(name);
            }
            Err(e) => lost.push((name, e)),
        }
    }
    let bytes = out.finish().map(|c| c.into_inner()).unwrap_or_default();
    Salvage { bytes, kept, lost }
}

/// deflate を解く。**途中で切れていても、解けた分までを返します。**
fn resolve_it(body: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut d = flate2::read::DeflateDecoder::new(body);
    let mut out = Vec::new();
    match d.read_to_end(&mut out) {
        Ok(_) => Ok(out),
        // 途中で切れた: 解けた分があればそれを返す(空なら捨てる)
        Err(e) if !out.is_empty() => {
            let _ = e;
            Ok(out)
        }
        Err(e) => Err(format!("解けません({e})")),
    }
}
