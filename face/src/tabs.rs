//! **文章と表のリボンの段を突き合わせる**(SEKKEI「画面を1つにする」5段目、
//! 2026-08-19 発注者「calc と writer のリボンの一覧で、相互関係をつくらないと」)。
//!
//! どちらの画面でも段を**同じ並び**で出し、その画面に無い段は灰色にします。
//! 並びが動かないので、画面が変わっても段を探し直さずに済みます。
//!
//! *対応は書きません。走るたびに表から作ります。*
//! [`crate::ribbon`] は `gen_ribbon.py` の生成物で「手で書かない」ファイル
//! です。そこに対応表を書くと、**次に生成した時に消えます**。だからここ
//! (手で書く場所)に置き、突き合わせは表そのものから作ります。
//!
//! 突き合わせは**段の名前**で行います。1つのファイルの中では文章の段も表の
//! 段も同じ言語なので(同じ `locale` から起こしているため)、共通の段は
//! 文字が一致します。実際に一致することは試験で縛ってあります。

use crate::ribbon;

/// 段1つぶんの居場所。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// 画面に出す見出し
    pub name: &'static str,
    /// 文章の段の番号(`None` は文章の画面に無い段)
    pub doc: Option<usize>,
    /// 表の段の番号(`None` は表の画面に無い段)
    pub sheet: Option<usize>,
}

/// **段の並びを1本にする。**
///
/// 並びは*文章を軸*にして、表だけの段をレイアウトの後ろへ入れます。
/// 文章にも表にも無い段はありません(どちらかの表から来るため)。
pub fn merged() -> Vec<Slot> {
    let w = ribbon::writer_tabs();
    let c = ribbon::calc_tabs();
    let 表の番号 = |名: &str| c.iter().position(|t| t.name == 名);

    let mut out: Vec<Slot> = w
        .iter()
        .enumerate()
        .map(|(i, t)| Slot { name: t.name, doc: Some(i), sheet: 表の番号(t.name) })
        .collect();

    // 表だけの段を、レイアウトの後ろへ順に差し込みます。文章の並びは
    // 動かしません — 使う人が覚えた場所を変えないためです
    let mut at = w
        .iter()
        .position(|t| 表の番号(t.name).is_some() && t.name != "ファイル")
        .map(|_| レイアウトの次(&out))
        .unwrap_or(out.len());
    for (i, t) in c.iter().enumerate() {
        if w.iter().any(|x| x.name == t.name) {
            continue;
        }
        out.insert(at, Slot { name: t.name, doc: None, sheet: Some(i) });
        at += 1;
    }
    out
}

/// レイアウトの段の次の場所。見つからなければ文章の段の末尾。
///
/// 名前で探すのは翻訳に弱いので、**5番目**(ファイル・ホーム・挿入・描画・
/// レイアウト)を既定にし、段がそれより少なければ末尾にします。
fn レイアウトの次(out: &[Slot]) -> usize {
    5.min(out.len())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // **言語を引く道は、この試験の中でしか呼びません。**
    // `ribbon::writer_tabs()` は1プロセスに一度きりの初期化を通るので、
    // `settings` の言語の試験より先に呼ぶとあちらを落とします
    // (2026-08-19 に実際に落としました)。だからこの試験は
    // `cargo test -p face tabs::` のように**単独で**回します。

    /// 段の数。文章 11 + 表 13 のうち共通が 9 で、合わせて 15
    #[test]
    #[ignore = "言語の初期化を先に触るので、単独で回す(cargo test -p face tabs:: -- --ignored)"]
    fn 十五段になる() {
        let m = merged();
        assert_eq!(m.len(), 15, "段の数が合わない: {:?}", m.iter().map(|s| s.name).collect::<Vec<_>>());
        assert_eq!(m.iter().filter(|s| s.doc.is_some()).count(), 11);
        assert_eq!(m.iter().filter(|s| s.sheet.is_some()).count(), 13);
        assert_eq!(m.iter().filter(|s| s.doc.is_some() && s.sheet.is_some()).count(), 9);
    }

    /// **共通の段は文字が一致する。** ここが崩れると突き合わせが効かない
    #[test]
    #[ignore = "言語の初期化を先に触るので、単独で回す"]
    fn 共通の段は名前が一致する() {
        let m = merged();
        let 共通: Vec<&str> =
            m.iter().filter(|s| s.doc.is_some() && s.sheet.is_some()).map(|s| s.name).collect();
        assert!(共通.contains(&"ホーム"), "{共通:?}");
        assert!(共通.contains(&"表示"), "{共通:?}");
        assert_eq!(共通.len(), 9, "{共通:?}");
    }

    /// **元の段が1つ残らず出る。** 抜けると押せない段ができる
    #[test]
    #[ignore = "言語の初期化を先に触るので、単独で回す"]
    fn 元の段が全部出る() {
        let m = merged();
        let mut d: Vec<usize> = m.iter().filter_map(|s| s.doc).collect();
        let mut s: Vec<usize> = m.iter().filter_map(|x| x.sheet).collect();
        d.sort_unstable();
        s.sort_unstable();
        assert_eq!(d, (0..ribbon::writer_tabs().len()).collect::<Vec<_>>(), "文章の段に抜け");
        assert_eq!(s, (0..ribbon::calc_tabs().len()).collect::<Vec<_>>(), "表の段に抜け");
    }

    /// 文章の並びは動かない(使う人が覚えた場所を変えない)
    #[test]
    #[ignore = "言語の初期化を先に触るので、単独で回す"]
    fn 文章の並びは動かない() {
        let m = merged();
        for (i, _) in ribbon::writer_tabs().iter().enumerate().take(5) {
            assert_eq!(m[i].doc, Some(i), "{i} 番目がずれた");
        }
    }
}
