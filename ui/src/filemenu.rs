//! **ファイルのページの項目 — 表で持つ**(統合の段8 の1。2026-08-20)。
//!
//! 前は項目も押し先も画面の中に**その場の閉包**で書いてありました。
//! 「新規作成」「開く」「保存」…と20個ほど並び、うち 17 個は writer と calc で
//! 同じ id・同じ意味なのに、二度書かれていました。
//!
//! *この段では見た目を変えません。* 項目を表にし、押し先を1つの `match` に
//! 集めるだけです。**次の段で officework がこの表を読んでページを描きます** —
//! そのとき押し先が閉包のままだと、外から呼べません。
//!
//! ここが持つのは*何が並ぶか*だけで、*どう描くか*は画面の側にあります
//! (ファイルのページは全面で、色も組み方も画面ごとに違うため)。

/// ファイルのページの項目1つ。
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// 押されたときに画面へ渡す名前。**writer と calc で同じ物は同じ id**
    pub id: &'static str,
    /// 画面に出す見出し(訳し済み)
    pub label: String,
    /// 押せるか。**押せない物は灰色で出す** — できないことを、
    /// できるように見せない
    pub ready: bool,
    /// この項目の前に空きを入れる(組の区切り)
    pub gap: bool,
    /// いまこの面を出しているか(右に出ている面の項目を塗る)
    pub on: bool,
    /// 下へ寄せる(詳細設定・ヘルプ・機能のリクエスト)
    pub tail: bool,
}

impl Item {
    /// ふつうの項目。
    pub fn new(id: &'static str, label: impl Into<String>) -> Item {
        Item { id, label: label.into(), ready: true, gap: false, on: false, tail: false }
    }
    /// 押せない項目(灰色)。
    pub fn grey(mut self) -> Item {
        self.ready = false;
        self
    }
    /// 前に空きを入れる。
    pub fn gap(mut self) -> Item {
        self.gap = true;
        self
    }
    /// いまこの面を出している。
    pub fn on(mut self, v: bool) -> Item {
        self.on = v;
        self
    }
    /// 下へ寄せる。
    pub fn tail(mut self) -> Item {
        self.tail = true;
        self
    }
}


/// **ファイルのページの共通の腕が触る面**(統合の段8 の3)。
///
/// `appcmd::Screen` と同じ考え方です。**欄はここから増やさない** —
/// 増やすほど「画面の中身」が漏れて、officework へ移せなくなります。
pub trait FileScreen: crate::appcmd::Screen {
    /// 「‹ 戻る」— ファイルのページに来る前の段へ
    fn tab_to_prev(&mut self);
    /// 右に出す面を替える(0=詳細情報 1=最近開いた 2=詳細設定 3=フォルダから探す)
    fn set_file_view(&mut self, v: u8);
    /// いま開いているファイルの道
    fn opened(&self) -> Option<std::path::PathBuf>;

    // ---- 文書を扱うアプリの共通の動詞 ----
    //
    // **officework がページを描くときに呼ぶ口**でもあります(段8 の3)。
    // 中身はアプリの物(文書とブックでは保存の仕方が違う)ですが、
    // *何ができるか*は同じなので、名前を1つにします。
    /// 新しく作る。作れたら真(書きかけがあるときは断って偽)
    fn new_file(&mut self) -> bool;
    /// 開く窓を出す
    fn open_dialog_now(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
    /// **フォルダを開く窓を出す**(2026-08-25 発注者「どうしてフォルダーを
    /// 開くがないのだ」)。綴りはフォルダなので、仕事を替えるとはフォルダを
    /// 替えることです。前は*起動のときにしか*選べませんでした
    fn folder_dialog_now(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
    /// 上書き保存
    fn save_now(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
    /// 名前を付けて保存
    fn save_as_now(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
    /// 終わる(書きかけがあれば確認へ)
    fn quit_now(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
    /// 名前の付いた段へ移る。名前は骨組み(ribbon::skeleton。リボンは1つ)の英語("Protection" など)
    fn goto_tab_named(&mut self, name: &str);

    /// ファイルのページに**保護の一覧**を持っているか。持っていれば自分で
    /// その面へ切り替えて真を返します。持っていなければ偽で、リボンの
    /// 保護タブへ飛びます
    fn protect_page(&mut self) -> bool {
        false
    }
}

/// **共通の腕を捌く。** 捌いたら真、アプリの番なら偽。
///
/// 呼ぶ側は自分の `match` の**前**にこれを置きます。同じ id の腕を自分の側に
/// 残すと、こちらが先に取るので**残した腕は死にます**(`appcmd::run` と同じ作法)。
///
/// 2026-08-20 に数えたら、両方にある 14 の腕のうち **12 が中身まで同じ**でした。
/// 段8 の1 は写しを2つの `match` に整理し直しただけで、写しは減っていません。
/// **写しは揃わない** — この回だけで `帯`・一覧・版の控え・最近使った物が
/// 同じ形で食い違っていました。
pub fn run(s: &mut impl FileScreen, id: &str) -> bool {
    match id {
        "f-back" => {
            s.tab_to_prev();
            true
        }
        "f-info" => {
            s.set_file_view(0);
            true
        }
        "f-recent" => {
            s.set_file_view(1);
            true
        }
        "f-opts" => {
            s.set_file_view(2);
            true
        }
        "f-find" => {
            s.set_file_view(3);
            true
        }
        // **前に保存できずに終わった控えの一覧**(2026-08-21 の B-3)。
        // 中身はアプリが描きます — ここは面を切り替えるだけです
        "f-recover" => {
            s.set_file_view(4);
            true
        }
        // ファイルの置き場をデスクトップの道具で開く。**まだ名前が無ければ
        // そう言う** — 黙って何も起きないのが一番分からない
        "f-place" => {
            let msg = match s.opened().as_ref().and_then(|p| p.parent()) {
                Some(dir) => {
                    let d = dir.display().to_string();
                    match crate::open_outside(&d) {
                        crate::Opened::Yes => crate::tf!("opening", d).to_string(),
                        crate::Opened::JustNow => {
                            crate::t!("just_opened_give_window")
                                .to_string()
                        }
                        crate::Opened::Failed => {
                            crate::tf!("no_application_associated_file", d).to_string()
                        }
                    }
                }
                None => crate::t!("not_file_yet").to_string(),
            };
            s.say(msg);
            true
        }
        // 押せない項目(テンプレート・ヘルプ・機能のリクエスト)は何もしない
        "f-tpl" | "f-help" | "f-req" => true,
        _ => false,
    }
}

/// 共通の腕のうち、**窓の文脈が要る物**(2026-08-20)。
///
/// `run` と分けているのは `Context<Self>` が要るからです。呼ぶ側は
/// `run` の次にこれを置きます。
pub fn run_cx<S: FileScreen + Sized + 'static>(
    s: &mut S,
    id: &str,
    cx: &mut gpui::Context<S>,
) -> bool {
    match id {
        "f-new" => {
            if s.new_file() {
                s.tab_to_prev();
            }
            true
        }
        "f-open" => {
            s.tab_to_prev();
            s.open_dialog_now(cx);
            true
        }
        "f-folder" => {
            s.tab_to_prev();
            s.folder_dialog_now(cx);
            true
        }
        "f-save" => {
            s.save_now(cx);
            true
        }
        "f-saveas" => {
            s.save_as_now(cx);
            true
        }
        "f-quit" => {
            s.quit_now(cx);
            true
        }
        "f-protect" => {
            // 表の側はファイルのページに保護の一覧を持っています。文章の側は
            // まだ無いので、リボンの保護タブへ飛ばします
            // 段名は骨組み(ribbon::skeleton)の英語で引く。
            // 2026-08-26 に骨組みが英語になった後も「保護」で探していたので、
            // 文章の側は押しても何も起きなかった(2026-09-02 の突き合わせで発見)
            if !s.protect_page() {
                s.goto_tab_named("Protection");
            }
            true
        }
        _ => false,
    }
}


// ---- 左の列 ------------------------------------------------------------

/// 左の列の色。**画面のテーマから受け取ります**(ここでは決めない)。
pub struct SideLook {
    /// 列の地
    pub bg: gpui::Rgba,
    /// 右端の線
    pub border: gpui::Rgba,
    /// 押せる項目の字
    pub fg: gpui::Rgba,
    /// 押せない項目の字
    pub gray: gpui::Rgba,
    /// 乗ったとき・いま出している面の地
    pub hover: gpui::Rgba,
    /// 画面の拡大率(表は `us` を掛ける。文章は 1.0)
    pub scale: f32,
}

/// 場所の控え(点検の道具が座標を当てずに押せるように)。
pub type Boxes =
    std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, (f32, f32, f32, f32)>>>;

/// **左の列を描く**(統合の段8 の本体。2026-08-20)。
///
/// 項目の並びは `file_menu()` が、押したときの中身は `file_menu_click()` が
/// 持ちます。ここが持つのは*並べ方と見た目*だけです。
///
/// 呼ぶ側が3つあります — 文章の画面、表の画面、そして officework です。
/// 前は同じ 40 行が2箇所に写してあり、片方だけ直る型でした。
///
/// `boxes` を渡すと、項目1つずつの位置を控えます。渡さなければ控えません。
pub fn sidebar<V: gpui::Render>(
    look: &SideLook,
    items: &[Item],
    boxes: Option<Boxes>,
    cx: &mut gpui::Context<V>,
    on: impl Fn(&mut V, &'static str, &mut gpui::Context<V>) + Clone + 'static,
) -> gpui::Div {
    use gpui::prelude::*;
    use gpui::{div, px, SharedString};
    let s = look.scale;
    let (fg, gray, hover) = (look.fg, look.gray, look.hover);
    let mut sb = div()
        .w(px(280.0))
        .bg(look.bg)
        .border_r_1()
        .border_color(look.border)
        .flex()
        .flex_col()
        .py_2();
    let bottom_head = items.iter().position(|x| x.tail);
    for (k, it) in items.iter().enumerate() {
        if Some(k) == bottom_head {
            sb = sb.child(div().flex_1());
        } else if it.gap {
            sb = sb.child(div().h(px(10.0)));
        }
        let id = it.id;
        let mut d = div().id(id).px_4().py_1p5().text_size(px(s * 13.0));
        // **控えは最初の子に。** 最後に置くと流れの中で見出しの下に入り、
        // *1項目ぶん下の箱*を控えます(2026-08-17 に実際に踏んだ)
        if let Some(bx) = boxes.clone() {
            d = d.relative().child(
                gpui::canvas(
                    move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                        bx.borrow_mut().insert(
                            id,
                            (
                                f32::from(b.origin.x),
                                f32::from(b.origin.y),
                                f32::from(b.size.width),
                                f32::from(b.size.height),
                            ),
                        );
                    },
                    |_, _: (), _, _| {},
                )
                .absolute()
                .size_full(),
            );
        }
        let d = if it.ready {
            d.text_color(fg).cursor_pointer().hover(move |st| st.bg(hover))
        } else {
            d.text_color(gray)
        }
        .child(SharedString::from(it.label.clone()));
        let on = on.clone();
        let d = d.on_click(cx.listener(move |v, _, _, cx| {
            on(v, id, cx);
            cx.notify()
        }));
        sb = sb.child(if it.on { d.bg(hover) } else { d });
    }
    sb
}

// ---- 右の面 ------------------------------------------------------------

/// 右の面の色。**画面のテーマから受け取ります**(ここでは決めない)。
pub struct PaneLook {
    pub fg: gpui::Rgba,
    pub dim: gpui::Rgba,
    pub hover: gpui::Rgba,
    /// 画面の拡大率(表は `us` を掛ける。文章は 1.0)
    pub scale: f32,
}

/// 面の見出し。
pub fn pane_title(look: &PaneLook, t: &str) -> gpui::Div {
    use gpui::prelude::*;
    gpui::div()
        .text_size(gpui::px(look.scale * 16.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(look.fg)
        .child(gpui::SharedString::from(t.to_string()))
}

/// 「最近開いた」が空のときの一言。
pub fn recent_empty(look: &PaneLook) -> gpui::Div {
    use gpui::prelude::*;
    gpui::div()
        .text_color(look.dim)
        .child(crate::t!("none_yet_opening_saving"))
}

/// 「最近開いた」の1行(名前と、その置き場)。
///
/// **押す結び付けは付いていません** — 呼ぶ側が `.on_click(cx.listener(…))`
/// を足します(`ui::filelist::row` と同じ作法)。
pub fn recent_row(look: &PaneLook, i: usize, p: &std::path::Path) -> gpui::Stateful<gpui::Div> {
    use gpui::prelude::*;
    let s = look.scale;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let hover = look.hover;
    gpui::div()
        .id(gpui::SharedString::from(format!("recent-{i}")))
        .px_2()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .hover(move |st| st.bg(hover))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            gpui::div()
                .text_size(gpui::px(s * 13.0))
                .text_color(look.fg)
                .child(gpui::SharedString::from(name)),
        )
        .child(
            gpui::div()
                .text_size(gpui::px(s * 11.0))
                .text_color(look.dim)
                .child(gpui::SharedString::from(dir)),
        )
}


// ---- 詳細設定の面(統合の段8。2026-09-04)------------------------------

/// **詳細設定の1行の中の1つ。**
///
/// 押せる物(押すと値が変わる)と、見るだけの字の2つです。
/// 左から順に並びます — 文字の大きさの行は「−」「100%」「+」の3つです
#[derive(Clone, Debug, PartialEq)]
pub enum OptCell {
    /// 押すと変わる値。押しは `id` で画面へ返します。
    /// **`String` です** — 一覧の行は番号を含むため(`python:3`)
    Button { id: String, text: String },
    /// 見るだけの字
    Text(String),
}

/// **詳細設定の1行**(見出しと、その右に並ぶ物)。
#[derive(Clone, Debug, PartialEq)]
pub struct OptRow {
    pub label: String,
    pub cells: Vec<OptCell>,
    /// 前に空きを入れる(組の区切り)
    pub gap: bool,
}

impl OptRow {
    /// 押せる物が1つだけの行(いちばん多い形)。
    pub fn one(
        label: impl Into<String>,
        id: impl Into<String>,
        text: impl Into<String>,
    ) -> OptRow {
        OptRow {
            label: label.into(),
            cells: vec![OptCell::Button { id: id.into(), text: text.into() }],
            gap: false,
        }
    }
    /// 見るだけの行。
    pub fn view(label: impl Into<String>, text: impl Into<String>) -> OptRow {
        OptRow { label: label.into(), cells: vec![OptCell::Text(text.into())], gap: false }
    }
    /// 前に空きを入れる。
    pub fn gap(mut self) -> OptRow {
        self.gap = true;
        self
    }
}

/// 詳細設定の面の色と大きさ。
pub struct OptLook {
    /// 見出しの字
    pub dim: gpui::Rgba,
    /// 押せる物の下地
    pub chip: gpui::Rgba,
    pub scale: f32,
}

/// **詳細設定の面を描く**(統合の段8。2026-09-04)。
///
/// 何が並ぶかは画面が決め([`OptRow`] の列)、どう描くかはここが持ちます。
/// [`sidebar`] と同じ分け方です。
///
/// 前は writer と calc に同じ 220 行が写してあり、8行のうち7行までが
/// 同じ物でした(違うのは表の「参照形式」だけ)。**写しは揃いません** —
/// 片方に足した行が、もう片方から抜けたままになります。
pub fn options<V: gpui::Render>(
    look: &OptLook,
    title: &str,
    note: &str,
    rows: &[OptRow],
    // 場所の控え(点検の道具が座標を当てずに押せるように)。sidebar と同じ形
    boxes: Option<Boxes>,
    cx: &mut gpui::Context<V>,
    on: impl Fn(&mut V, String, &mut gpui::Context<V>) + Clone + 'static,
) -> gpui::Div {
    use gpui::prelude::*;
    use gpui::{div, px, SharedString};
    let s = look.scale;
    let mut pane = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_size(px(s * 16.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(SharedString::from(title.to_string())),
        )
        .child(div().text_color(look.dim).child(SharedString::from(note.to_string())))
        .child(div().h(px(s * 6.0)));
    for r in rows {
        if r.gap {
            pane = pane.child(div().h(px(s * 10.0)));
        }
        let mut line = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(s * 200.0))
                    .text_color(look.dim)
                    .child(SharedString::from(r.label.clone())),
            );
        for c in &r.cells {
            line = match c {
                OptCell::Text(t) => line.child(div().child(SharedString::from(t.clone()))),
                OptCell::Button { id, text } => {
                    let id = id.clone();
                    let on = on.clone();
                    // **押せる物は場所を控えます**(実機の点検のため)。
                    // 控えの鍵は `&'static str` なので、一覧の行(`python:3`)は
                    // 番号ごとに1つずつ漏れないよう、控える数を絞ります
                    let mark = boxes.clone().map(|rec| {
                        let key: Option<&'static str> = match id.as_str() {
                            "set-lang" => Some("set-lang"),
                            "set-theme" => Some("set-theme"),
                            "set-ui-minus" => Some("set-ui-minus"),
                            "set-ui-plus" => Some("set-ui-plus"),
                            "set-username" => Some("set-username"),
                            "set-iter" => Some("set-iter"),
                            "set-refstyle" => Some("set-refstyle"),
                            "set-autocorrect" => Some("set-autocorrect"),
                            "set-ai" => Some("set-ai"),
                            "set-python" => Some("set-python"),
                            // 一覧の行は先頭の3つだけ控えます(点検はそれで足ります)
                            "python:0" => Some("python:0"),
                            "python:1" => Some("python:1"),
                            "python:2" => Some("python:2"),
                            _ => None,
                        };
                        gpui::canvas(
                            move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                                if let Some(k) = key {
                                    rec.borrow_mut().insert(
                                        k,
                                        (
                                            f32::from(b.origin.x),
                                            f32::from(b.origin.y),
                                            f32::from(b.size.width),
                                            f32::from(b.size.height),
                                        ),
                                    );
                                }
                            },
                            |_, _: (), _, _| {},
                        )
                        .absolute()
                        .size_full()
                    });
                    line.child(
                        div()
                            .id(SharedString::from(id.clone()))
                            .relative()
                            .children(mark)
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .bg(look.chip)
                            .child(SharedString::from(text.clone()))
                            .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                                on(this, id.clone(), cx);
                                cx.notify()
                            })),
                    )
                }
            };
        }
        pane = pane.child(line);
    }
    pane
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn the_default_built_item_is_an_ordinary_clickable_one() {
        let i = Item::new("f-new", "新規作成");
        assert!(i.ready && !i.gap && !i.on && !i.tail);
    }

    #[test]
    fn greyed_spacer_and_bottom_can_be_combined() {
        let i = Item::new("f-help", "ヘルプ").grey().gap().tail();
        assert!(!i.ready && i.gap && i.tail);
    }

    #[test]
    fn an_option_row_holds_a_label_and_what_sits_beside_it() {
        let r = OptRow::one("言語", "set-lang", "日本語").gap();
        assert!(r.gap);
        assert_eq!(r.cells, vec![OptCell::Button { id: "set-lang".into(), text: "日本語".into() }]);
        let v = OptRow::view("書体の置き場", "/usr/share/fonts");
        assert_eq!(v.cells, vec![OptCell::Text("/usr/share/fonts".into())]);
        assert!(!v.gap);
    }
}
