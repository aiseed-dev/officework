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

/// **一覧に「この文書の標準の大きさ」を差し込む**(2026-08-20 発注者
/// 「日本語は 10.5 を選択できるように」→「テンプレートで対応するのがいい」)。
///
/// 言語では分けません。文書の標準はテンプレートが決め(既定は 10.5pt —
/// `kumihan::DEFAULT_PT`)、その値が並びに無ければ足して出します。
/// 日本語の既定テンプレートの文書では 10.5 が一覧に出て、+/− も止まります。
/// 一覧と +/− は必ず同じ並びを見ます(食い違うと、一覧で選べた値に
/// +/− が止まりません)。
pub fn sizes_with(standard: Option<f32>) -> Vec<f32> {
    let mut v: Vec<f32> = SIZES.to_vec();
    if let Some(s) = standard {
        if !v.iter().any(|&x| (x - s).abs() < 0.05) {
            v.push(s);
            v.sort_by(|a, b| a.partial_cmp(b).expect("大きさに NaN は来ない"));
        }
    }
    v
}

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

/// +/−(incfont/decfont)。渡した一覧の値を**1段ずつ**辿る。
///
/// - `up=true` で1つ大きい一覧値へ、`false` で1つ小さい一覧値へ
/// - 半端な値(一覧に無い値)は、動く向きの**隣の一覧値へ寄る**
///   (大きくするなら「今より大きい最初の一覧値」)
/// - 端では止まる(72 で大きくしても 72、6 で小さくしても 6)
///
/// 本家もこう動く — ±1pt は我流の間違いだった(ui.ja.md の基準の節)。
/// アプリは [`sizes_with`](文書の標準入り)の並びを渡す。
pub fn step_size_in(list: &[f32], cur: f32, up: bool) -> f32 {
    if up {
        // 今より大きい最初の一覧値。無ければ(=最大以上)今のまま
        list.iter().copied().find(|&s| s > cur + f32::EPSILON).unwrap_or(cur)
    } else {
        // 今より小さい最後の一覧値。無ければ(=最小以下)今のまま
        list.iter().rev().copied().find(|&s| s < cur - f32::EPSILON).unwrap_or(cur)
    }
}

/// [`step_size_in`] の [`SIZES`] 版。標準の差し込みが要らない所と試験が使う。
pub fn step_size(cur: f32, up: bool) -> f32 {
    step_size_in(SIZES, cur, up)
}

/// [`sizes_with`] の並びを1段辿る。値だけ持ち込めばよいので、
/// `Copy` が要るクロージャ(writer の `size()`)からも呼べる。
pub fn step_size_with(standard: Option<f32>, cur: f32, up: bool) -> f32 {
    step_size_in(&sizes_with(standard), cur, up)
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

// ---- 一覧を出す位置 ----------------------------------------------------
//
// **表の画面から移しました**(2026-08-20)。文章の画面の一覧も
// ボタンの真下に出す決めなので(2026-08-15 発注者)、片方に置いたままだと
// もう片方が写しを持つことになります。ここは絵を描かない層なので、
// gpui を持ち込みません。

/// リボンから開く一覧の幅。書体名は長いので、セルの列幅ではなくこの幅。
pub const POP_W: f32 = 240.0;

/// **リボンのボタンから開く一覧を、そのボタンの真下に出す。**
///
/// リボンで書体を変えようとすると、一覧が押したボタンではなく**選んで
/// いるセルの下**に出ていた(発注者報告 2026-08-08)。ボタンは画面の
/// 一番上、一覧は画面の真ん中 — 目が二往復する。
///
/// `btn`・`pane` はどちらも窓の座標での (x, y, 幅, 高さ)。返す値は
/// **格子の面を基準にした座標**(一覧はその面の中に置かれるため)。
///
/// 横はボタンの左端にそろえ、右端からはみ出すぶんだけ内へ寄せる。幅は
/// 中身でまちまちなので POP_W で見る。面の幅がまだ分からない(一度も
/// 描いていない)ときは寄せない。
///
/// **縦はボタンの真下。**(2026-08-15 に直した)
///
/// 前は「面の一番上まで」で頭打ちにしていた — 一覧を置く層が格子の面の
/// 中にあり `overflow_hidden` で切られるためで、その註に「面より上へ出す
/// には一覧の層を窓の根に移す必要があり、それは別途」と書いてあった。
/// **その別途をやった** — 一覧・品書き・罫線のパレットを窓の根へ移し、
/// 描くときに面の原点を足す形にした。だからここは**負を返してよい**
/// (リボンは面より上にあるので、リボンから開くときは必ず負になる)。
///
/// 発注者 2026-08-15「ドロップダウンのリストは、テキスト表示のすぐ下に
/// 出したほうがいい」。実測では書体の欄(下辺 90)から開いた一覧が
/// y=2、つまり数式バーを挟んだ 80px 下に出ていた。
pub fn pop_under(btn: (f32, f32, f32, f32), pane: (f32, f32, f32, f32)) -> (f32, f32) {
    let (bx, by, _, bh) = btn;
    let (px0, py0, pw, _) = pane;
    (pop_x(bx - px0, pw), by + bh + 2.0 - py0)
}

/// 一覧が痩せてよい下限。これを割るなら、真下に出すのをあきらめて
/// 内へ寄せる(書体名は長いので、狭い一覧は読めず用を成さない)
pub const POP_MIN_W: f32 = 160.0;

/// **一覧の左端。** ボタンの左にそろえるのが基本で、右端に寄りすぎて
/// 一覧が [`POP_MIN_W`] を割るときだけ内へ寄せる。
///
/// 前は「幅は分からないので上限 [`POP_W`] で見る」として、右から 240px
/// 以内のボタンを一律に寄せていた。実測(2026-08-15、ribbon_sweep)では
/// `cell-styles` の一覧が本当は 121px しかないのに 68px 左へずれており、
/// **真下に出すという約束のほうが先に破れていた**。幅は描くときに
/// 「窓に残っている幅」で頭打ちにするので、ここで寄せる必要は薄い。
pub fn pop_x(x: f32, pane_w: f32) -> f32 {
    if pane_w <= 0.0 {
        return x.max(0.0);
    }
    if pane_w - x >= POP_MIN_W {
        x.max(0.0)
    } else {
        (pane_w - POP_W).max(0.0)
    }
}

/// **幅の分かっている一覧の横位置**(記号の升の並びなど)。
/// 右端からはみ出す分だけ内へ寄せる。[`pop_x`] は幅の読めない一覧向けで
/// 上限 [`POP_W`] で見るため、それより広い格子は右端で切れてしまう
/// (2026-08-21 に実機で見た)。
pub fn pop_x_w(x: f32, pane_w: f32, w: f32) -> f32 {
    if pane_w <= 0.0 {
        return x.max(0.0);
    }
    x.min(pane_w - w - 8.0).max(0.0)
}

/// **一覧を上に出すか下に出すか、高さの上限はいくつか。**
///
/// 発注者 2026-08-15「場所によっては上に出さないといけなかったり、
/// 上下に出す場合もある」。決め:
///
/// - 下に**入るなら下**(手が下りていくのと同じ向きで自然)
/// - 入らないなら、**広いほうの側**へ出す
/// - どちらにも入りきらないときは広いほうに出して、残りは中で送る
///   (数で切り捨てない)
///
/// 引数はすべて**窓の座標**。`top`/`bottom` は開く元(ボタンや欄)の
/// 上辺と下辺、`want_h` は中身を全部出すのに要る高さ。
///
/// 返り `(上に出すか, 位置, 高さの上限)`。位置は下に出すなら窓の上端から
/// の距離、**上に出すなら窓の下端からの距離** — 上に出すときは下辺を
/// ボタンに合わせたいので、中身が短くても隙間が空かないようにする。
pub fn pop_place(top: f32, bottom: f32, want_h: f32, win_h: f32) -> (bool, f32, f32) {
    const GAP: f32 = 2.0; // 開く元との隙間
    const EDGE: f32 = 8.0; // 窓の端に貼り付けない
    let below = (win_h - EDGE) - (bottom + GAP);
    let above = (top - GAP) - EDGE;
    if want_h <= below || below >= above {
        (false, bottom + GAP, below.max(0.0))
    } else {
        (true, win_h - (top - GAP), above.max(0.0))
    }
}

/// ボタンの場所がまだ分からないとき(描く前に鍵で呼ばれた等)の逃げ道。
/// 押した点を左端と見なして同じように寄せる。
pub fn pop_at_click(click_x: f32, pane: (f32, f32, f32, f32)) -> (f32, f32) {
    pop_under((click_x - 12.0, pane.1 - 2.0, 0.0, 0.0), pane)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_keeps_only_what_contains_the_typed_text() {
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
    fn filtering_keeps_the_original_order() {
        let items = [("あ", "a"), ("あい", "ai"), ("あいう", "aiu")];
        assert_eq!(filter(&items, "あ"), vec![0, 1, 2]);
        assert_eq!(filter(&items, "あい"), vec![1, 2]);
    }

    #[test]
    fn sizes_round_to_half_steps_from_4_to_409() {
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
    fn plus_minus_walks_the_list_one_step() {
        // 一覧値からは隣の一覧値へ
        assert_eq!(step_size(11.0, true), 12.0);
        assert_eq!(step_size(12.0, false), 11.0);
        assert_eq!(step_size(28.0, true), 36.0);
        assert_eq!(step_size(36.0, false), 28.0);
    }

    #[test]
    fn an_odd_value_snaps_to_the_neighbour_in_that_direction() {
        // 13pt は一覧に無い(12 と 14 の間)。大きくすると 14、小さくすると 12
        assert_eq!(step_size(13.0, true), 14.0);
        assert_eq!(step_size(13.0, false), 12.0);
        // 11.5pt は 11 と 12 の間
        assert_eq!(step_size(11.5, true), 12.0);
        assert_eq!(step_size(11.5, false), 11.0);
    }

    #[test]
    fn stops_at_either_end() {
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
    fn inserts_the_document_default_into_the_list() {
        // 既定テンプレート(10.5)の文書では 10.5 が並びに入る。場所は 10 と 11 の間
        let v = sizes_with(Some(10.5));
        let i = v.iter().position(|&x| x == 10.5).expect("10.5 が入る");
        assert_eq!(v[i - 1], 10.0);
        assert_eq!(v[i + 1], 11.0);
        assert_eq!(v.len(), SIZES.len() + 1);
        // 並びに既にある値なら足さない
        assert_eq!(sizes_with(Some(11.0)), SIZES.to_vec());
        // 指定なしなら Excel の並びのまま
        assert_eq!(sizes_with(None), SIZES.to_vec());
    }

    #[test]
    fn plus_minus_stops_on_the_inserted_default() {
        let v = sizes_with(Some(10.5));
        assert_eq!(step_size_in(&v, 10.0, true), 10.5);
        assert_eq!(step_size_in(&v, 10.5, true), 11.0);
        assert_eq!(step_size_in(&v, 11.0, false), 10.5);
        // 差し込みが無ければ 10 の次は 11(今までどおり)
        assert_eq!(step_size_in(SIZES, 10.0, true), 11.0);
    }

    #[test]
    fn reports_where_the_current_value_sits() {
        let items = [("11", "11"), ("12", "12"), ("14", "14")];
        assert_eq!(current_index(&items, "12"), 1);
        // 合致が無ければ先頭
        assert_eq!(current_index(&items, "13"), 0);
    }

    #[test]
    fn whole_sizes_show_without_a_decimal_point() {
        assert_eq!(size_label(11.0), "11");
        assert_eq!(size_label(11.5), "11.5");
        assert_eq!(size_label(10.5), "10.5");
    }
}
