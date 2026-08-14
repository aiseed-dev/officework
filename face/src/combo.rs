//! 「開く物」— コンボと絞り込みの**純粋な芯**。
//!
//! 画面(gpui)から切り離した所に、絞り込み・大きさの丸め・大きさの1段送りを
//! 純関数で置く。calc も writer も**同じ判断**をここから借りる — 見た目の骨組みは
//! それぞれのアプリに残るが、「打つほど絞る」「4〜409pt に丸める」「+/− で一覧を
//! 1段辿る」の中身は1箇所。ここは単体で試せる(gpui を呼ばない)。
//!
//! 部品の種類はいまは2つ:
//!   - 素の一覧([`Kind::Plain`]) — 固定の短い一覧(大きさ・揃え)。絞り込み無し
//!   - 絞り込みつき([`Kind::Filter`]) — 数が増える一覧(書体・入力規則)。打つほど絞る
//!
//! パレット・枝分かれ・2段への全面移行はここではやらない(後で寄せられる形)。

/// 一覧の種類。**打鍵の挙動が変わる** — 素の一覧は打つと一覧が閉じるが、
/// 絞り込みつきは打つと絞る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 固定の短い一覧。絞り込みは要らない
    Plain,
    /// 数が増える一覧。打つほど絞る
    Filter,
}

/// 大きさの一覧(pt)。**この17個が正**。calc のリボンの欄・+/− の1段送り・
/// 試験がすべてここを見る(1箇所に集める — 綴りがずれる余地を作らない)。
/// Excel の標準の並び。
pub const SIZES: &[f32] = &[
    6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0,
    72.0,
];

/// 大きさの自由入力を**画面の入り口でだけ**丸める境。
/// 模型(sheet)と Python の口には掛けない — よそで作った 2pt は 2pt のまま往復する。
pub const SIZE_MIN: f32 = 4.0;
pub const SIZE_MAX: f32 = 409.0;
/// 丸めの刻み(0.5pt)
pub const SIZE_STEP: f32 = 0.5;

/// 絞り込み。**打った字を含む物だけ残す**(並びは元のまま — 崩さない)。
///
/// 照合は各項の**鍵と副名の両方**を見る(書体の日本語名と英語名など)。
/// 大文字小文字・前後の空白は無視する。`query` が空なら全部返す。
///
/// 返すのは元の一覧の**添字**の列 — 呼ぶ側が (鍵, 見出し) など好きな組を
/// 持っているので、添字で引かせる。
pub fn filter(items: &[(&str, &str)], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, (key, alt))| {
            q.is_empty()
                || key.to_lowercase().contains(&q)
                || alt.to_lowercase().contains(&q)
        })
        .map(|(i, _)| i)
        .collect()
}

/// 大きさを 4〜409pt・0.5 刻みに**黙って丸める**。断り立てはしない。
///
/// **画面の入力にだけ掛ける。** 模型と Python の口には渡さない。
/// 4 未満は 4 に、409 超は 409 に寄せる。
pub fn round_size(pt: f32) -> f32 {
    let clamped = pt.clamp(SIZE_MIN, SIZE_MAX);
    (clamped / SIZE_STEP).round() * SIZE_STEP
}

/// +/−(incfont/decfont)。一覧([`SIZES`])の値を**1段ずつ**辿る。
///
/// - `up=true` で1つ大きい一覧値へ、`false` で1つ小さい一覧値へ
/// - 半端な値(一覧に無い値)は、動く向きの**隣の一覧値へ寄る**
///   (大きくするなら「今より大きい最初の一覧値」)
/// - 端では止まる(72 で大きくしても 72、6 で小さくしても 6)
///
/// 本家もこう動く — ±1pt は我流の間違いだった(ui.ja.md の基準の節)。
pub fn step_size(cur: f32, up: bool) -> f32 {
    if up {
        // 今より大きい最初の一覧値。無ければ(=最大以上)今のまま
        SIZES
            .iter()
            .copied()
            .find(|&s| s > cur + f32::EPSILON)
            .unwrap_or(cur)
    } else {
        // 今より小さい最後の一覧値。無ければ(=最小以下)今のまま
        SIZES
            .iter()
            .rev()
            .copied()
            .find(|&s| s < cur - f32::EPSILON)
            .unwrap_or(cur)
    }
}

/// 一覧を開いたときに**今の値の位置**を出す。合致が無ければ 0(先頭)。
///
/// 照合は鍵で引く(見出し=訳では引かない)。書体名・大きさの文字列など、
/// 「今これが効いている」項へ選択を送って開く。
pub fn current_index(items: &[(&str, &str)], current: &str) -> usize {
    items
        .iter()
        .position(|(key, _)| *key == current)
        .unwrap_or(0)
}

/// 大きさを画面に出す字にする。整数なら小数点を出さない(11 / 11.5)。
pub fn size_label(pt: f32) -> String {
    if (pt - pt.round()).abs() < 0.05 {
        format!("{}", pt.round() as u32)
    } else {
        format!("{pt:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 絞り込みは打った字を含む物だけ残す() {
        let items = [("游ゴシック", "Yu Gothic"), ("メイリオ", "Meiryo"), ("MS 明朝", "MS Mincho")];
        // 日本語名で引く
        assert_eq!(filter(&items, "ゴシック"), vec![0]);
        // 英語名(副名)でも引く
        assert_eq!(filter(&items, "meiryo"), vec![1]);
        // 大文字小文字は無視
        assert_eq!(filter(&items, "MINCHO"), vec![2]);
        // 空なら全部
        assert_eq!(filter(&items, ""), vec![0, 1, 2]);
        // 前後の空白は無視
        assert_eq!(filter(&items, "  ms  "), vec![2]);
    }

    #[test]
    fn 絞り込みは元の並びを崩さない() {
        let items = [("あ", "a"), ("あい", "ai"), ("あいう", "aiu")];
        assert_eq!(filter(&items, "あ"), vec![0, 1, 2]);
        assert_eq!(filter(&items, "あい"), vec![1, 2]);
    }

    #[test]
    fn 大きさは四から四百九の半刻みに丸める() {
        assert_eq!(round_size(11.0), 11.0);
        assert_eq!(round_size(11.2), 11.0);
        assert_eq!(round_size(11.3), 11.5);
        assert_eq!(round_size(11.7), 11.5);
        assert_eq!(round_size(11.8), 12.0);
        // 端を越えたら寄せる
        assert_eq!(round_size(2.0), 4.0);
        assert_eq!(round_size(0.0), 4.0);
        assert_eq!(round_size(500.0), 409.0);
        // 端そのもの
        assert_eq!(round_size(4.0), 4.0);
        assert_eq!(round_size(409.0), 409.0);
    }

    #[test]
    fn プラスマイナスは一覧を一段ずつ辿る() {
        // 一覧値からは隣の一覧値へ
        assert_eq!(step_size(11.0, true), 12.0);
        assert_eq!(step_size(12.0, false), 11.0);
        assert_eq!(step_size(28.0, true), 36.0);
        assert_eq!(step_size(36.0, false), 28.0);
    }

    #[test]
    fn 半端な値は動く向きの隣の一覧値へ寄る() {
        // 13pt は一覧に無い(12 と 14 の間)。大きくすると 14、小さくすると 12
        assert_eq!(step_size(13.0, true), 14.0);
        assert_eq!(step_size(13.0, false), 12.0);
        // 11.5pt は 11 と 12 の間
        assert_eq!(step_size(11.5, true), 12.0);
        assert_eq!(step_size(11.5, false), 11.0);
    }

    #[test]
    fn 端では止まる() {
        // 最大で大きくしても最大のまま
        assert_eq!(step_size(72.0, true), 72.0);
        // 最大より大きい半端でも止まる
        assert_eq!(step_size(100.0, true), 100.0);
        assert_eq!(step_size(100.0, false), 72.0);
        // 最小で小さくしても最小のまま
        assert_eq!(step_size(6.0, false), 6.0);
        // 最小より小さい半端でも止まる
        assert_eq!(step_size(5.0, false), 5.0);
        assert_eq!(step_size(5.0, true), 6.0);
    }

    #[test]
    fn 今の値の位置を出す() {
        let items = [("11", "11"), ("12", "12"), ("14", "14")];
        assert_eq!(current_index(&items, "12"), 1);
        // 合致が無ければ先頭
        assert_eq!(current_index(&items, "13"), 0);
    }

    #[test]
    fn 大きさの見せ字は整数なら小数点を出さない() {
        assert_eq!(size_label(11.0), "11");
        assert_eq!(size_label(11.5), "11.5");
        assert_eq!(size_label(10.5), "10.5");
    }
}
