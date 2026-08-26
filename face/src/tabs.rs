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
        // **ファイルの段は名前で書きません**(2026-08-26)。表はその言語の
        // 物なので、"ファイル" と書くと英語の画面では当たりません。
        // ファイルは先頭と決まっているので、同じ表の先頭と比べます
        .position(|t| 表の番号(t.name).is_some() && Some(t.name) != w.first().map(|x| x.name))
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

    // **前は単独で回していました**(2026-08-21 に直しました)。
    // `ribbon::writer_tabs()` は言語の控えを埋めるので、環境変数から
    // 言語を決める試験より先に走ると、あちらを落としていました。
    // その試験を `lang::i18n`(控えを直に触れる場所)へ移したので、
    // 順番の縛りが要らなくなり、`#[ignore]` を外しました。

    /// 段の数。文章 11 + 表 13 のうち共通が 9 で、合わせて 15
    #[test]
    fn 十五段になる() {
        let m = merged();
        assert_eq!(m.len(), 15, "段の数が合わない: {:?}", m.iter().map(|s| s.name).collect::<Vec<_>>());
        assert_eq!(m.iter().filter(|s| s.doc.is_some()).count(), 11);
        assert_eq!(m.iter().filter(|s| s.sheet.is_some()).count(), 13);
        assert_eq!(m.iter().filter(|s| s.doc.is_some() && s.sheet.is_some()).count(), 9);
    }

    /// **共通の段は文字が一致する。** ここが崩れると突き合わせが効かない。
    ///
    /// *日本語の段名で検べません*(2026-08-21)。言語はいつでも替えられる
    /// ので、「ホームがある」と書くと言語を替える試験と取り合います。
    /// 代わりに**両方の表の同じ場所を突き合わせます** — どの言語でも
    /// 成り立つ言い方です。
    #[test]
    fn 共通の段は名前が一致する() {
        let m = merged();
        let 共通: Vec<&Slot> =
            m.iter().filter(|s| s.doc.is_some() && s.sheet.is_some()).collect();
        assert_eq!(共通.len(), 9, "{:?}", 共通.iter().map(|s| s.name).collect::<Vec<_>>());
        for s in 共通 {
            let d = ribbon::writer_tabs()[s.doc.expect("文章の段")].name;
            let c = ribbon::calc_tabs()[s.sheet.expect("表の段")].name;
            assert_eq!(d, c, "共通の段なのに名前が違う");
            assert_eq!(s.name, d, "まとめた表の名前が元と違う");
        }
    }

    /// **元の段が1つ残らず出る。** 抜けると押せない段ができる
    #[test]
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
    fn 文章の並びは動かない() {
        let m = merged();
        for (i, _) in ribbon::writer_tabs().iter().enumerate().take(5) {
            assert_eq!(m[i].doc, Some(i), "{i} 番目がずれた");
        }
    }
}
