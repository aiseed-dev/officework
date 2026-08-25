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
    /// 名前の付いた段へ移る(「保護」など)
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
                        crate::Opened::Yes => crate::tf!("Opening: {}", d).to_string(),
                        crate::Opened::JustNow => {
                            crate::t!("Just opened it (give the window a moment to appear)")
                                .to_string()
                        }
                        crate::Opened::Failed => {
                            crate::tf!("No application is associated with this file: {}", d).to_string()
                        }
                    }
                }
                None => crate::t!("Not a file yet").to_string(),
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
            if !s.protect_page() {
                s.goto_tab_named("保護");
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
    let 下寄せの頭 = items.iter().position(|x| x.tail);
    for (k, it) in items.iter().enumerate() {
        if Some(k) == 下寄せの頭 {
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
        .child(crate::t!("(none yet; opening and saving adds entries)"))
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 組み立ての既定は押せる普通の項目() {
        let i = Item::new("f-new", "新規作成");
        assert!(i.ready && !i.gap && !i.on && !i.tail);
    }

    #[test]
    fn 灰色と空きと下寄せを重ねられる() {
        let i = Item::new("f-help", "ヘルプ").grey().gap().tail();
        assert!(!i.ready && i.gap && i.tail);
    }
}
