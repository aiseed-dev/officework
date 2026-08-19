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
        // ファイルの置き場をデスクトップの道具で開く。**まだ名前が無ければ
        // そう言う** — 黙って何も起きないのが一番分からない
        "f-place" => {
            let msg = match s.opened().as_ref().and_then(|p| p.parent()) {
                Some(dir) => {
                    let d = dir.display().to_string();
                    match crate::open_outside(&d) {
                        crate::Opened::Yes => crate::tf!("開きます: {}", d).to_string(),
                        crate::Opened::JustNow => {
                            crate::t!("さっき開きました(窓が出るまで少し待ってください)")
                                .to_string()
                        }
                        crate::Opened::Failed => {
                            crate::tf!("開けません(xdg-open がありません): {}", d).to_string()
                        }
                    }
                }
                None => crate::t!("まだファイルになっていません").to_string(),
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
            s.goto_tab_named("保護");
            true
        }
        _ => false,
    }
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
