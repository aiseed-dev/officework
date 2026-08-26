//! **共通の命令 — 同じ id は同じ場所で捌く**(2026-08-19 発注者「リボンの
//! ボタンを統一したら、コードも統一の方向にできるのでは」)。
//!
//! いままで同じ命令を writer と calc が別々に捌いていました。写しは必ず
//! ずれます — 実際、`ai-where` は writer だけ訳を通し、calc は `format!` の
//! ままで**13言語で日本語が出る**状態でした。ここに1本にすれば、直しも
//! 訳も一度で済みます。
//!
//! *命令は3種類に分かれます。*
//!
//! 1. **中身がアプリに依らない物**(ここで捌く)— 拡大・AI の宛先など、
//!    アプリの状態を同じ形で触るだけの命令
//! 2. *意味は同じで、触る先が違う物*(各アプリに残す)— 太字は文章では
//!    選んだ字に、表では選んだセルに掛かる。id は同じでも本体は別
//! 3. *その画面にしか無い物*(各アプリに残す)— 目次・ピボットなど
//!
//! **文言もここに置きます。** 走査(`ui/gen_i18n.py`)は `calc/src`
//! `writer/src` `ui/src` を見るので、`face` に置くと生きている訳が
//! 「使われていない」と数えられます。だから置き場は `ui` です。

/// 編集画面が差し出す面。**共通の命令が触る物だけ**を並べます。
///
/// 命令を1つ移すたびに、ここへ欄が1つ増えます。増えすぎたら
/// (アプリの状態そのものを共通の型にする)次の段の合図です。
pub trait Screen {
    /// 画面の倍率(1.0 が 100%)
    fn zoom_mut(&mut self) -> &mut f32;
    /// 画面が暗い側か
    fn dark_mut(&mut self) -> &mut bool;
    /// 画面の文字の大きさ(1.0 が 100%)。**紙やセルの大きさとは別**
    fn ui_scale_mut(&mut self) -> &mut f32;
    /// 状態の行へ1文
    fn say(&mut self, msg: String);
}

/// 共通の命令なら捌いて真を返します。偽ならアプリの番です。
///
/// 呼ぶ側は自分の `match` の**前**にこれを置きます。同じ id の腕を
/// 自分の側に残すと、こちらが先に取るので**残した腕は死にます** —
/// 移したら腕は消してください。
pub fn run(s: &mut impl Screen, id: &str) -> bool {
    match id {
        // 拡大・縮小。**紙は変わらない** — 見る大きさだけの話。
        // 前は writer が黙って変え、calc だけ「ズーム {}%」と言っていた。
        // 言う方に揃える — 今の倍率が見えないと、戻したい時に困る
        "zoom-in" | "zoom-out" => {
            let z = s.zoom_mut();
            *z = if id == "zoom-in" { (*z + 0.1).min(2.0) } else { (*z - 0.1).max(0.5) };
            let pct = (*z * 100.0).round() as i32;
            let msg = crate::tf!("zoom", pct);
            s.say(msg.to_string());
            true
        }
        // AI の宛先を順に替える。**writer と calc で同じ言い回し**
        "ai-where" => {
            let next = crate::ai::backend().next();
            crate::ai::set_backend(next);
            let msg = match crate::ai::ready(next) {
                Ok(_) => crate::tf!("ai_destination_remembered", next.label()),
                Err(e) => {
                    crate::tf!("ai_destination_but_unavailable", next.label(), e)
                }
            };
            s.say(msg.to_string());
            true
        }
        // 倍率を 100% に戻す。文章にしか無かったので表にも足しました
        // (2026-08-21 の B-3)。上の拡大・縮小と対になる命令です
        "zoom100" => {
            *s.zoom_mut() = 1.0;
            s.say(crate::t!("back_100").to_string());
            true
        }
        // マクロの置き場をファイル管理で開く。**表にしか無かった**ので
        // 文章にも足しました(2026-08-21 の B-3)。置き場は `pyrun` の1本で、
        // どちらのアプリから開いても同じ場所です
        "py-folder" => {
            let dir = crate::pyedit::plugins_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.display().to_string();
            let msg = match crate::open_outside(&path) {
                crate::Opened::Yes => crate::tf!("opening", path),
                crate::Opened::JustNow => {
                    crate::t!("just_opened_give_window").into()
                }
                crate::Opened::Failed => {
                    crate::tf!("no_application_associated_file", path)
                }
            };
            s.say(msg.to_string());
            true
        }
        // 画面の文字の大きさ。**文章にはボタンがありませんでした**
        // (2026-08-21 発注者「双方でできるようにしたいです」)。表の腕が
        // そのままアプリに依らない形だったので、ここへ移して両方から使います。
        //
        // 紙やセルの大きさは変わりません — あちらは拡大・縮小の話です。
        "ui-bigger" | "ui-smaller" => {
            let step = if id == "ui-bigger" { 0.1 } else { -0.1 };
            let s2 = s.ui_scale_mut();
            // 上限は 150% — これ以上はパネルや欄の設えが崩れる(発注者 2026-08-07)
            *s2 = (((*s2 + step) * 10.0).round() / 10.0).clamp(0.8, 1.5);
            let pct = (*s2 * 100.0).round() as i32;
            // 試験では書かない(実利用者の settings.toml を汚さない)
            if !cfg!(test) {
                crate::settings::set("ui_scale", &format!("{:.1}", pct as f32 / 100.0));
            }
            let msg = crate::tf!("ui_text_size_opens", pct);
            s.say(msg.to_string());
            true
        }
        // 画面の明暗。**2つのアプリで中身が1文字も違いませんでした**
        // (2026-08-21 の B-2)。文章は `darkmode`、表は `theme` という
        // 別の id で、どちらも `toggle_dark` を呼ぶだけだったので、
        // id を `darkmode` に揃えてここへ移しました。
        //
        // `persist` を偽にするのは試験のときだけです。実際に
        // `settings.toml` を書き替えてしまうと、試験が発注者の設定を壊します。
        "darkmode" | "theme" => {
            let cur = *s.dark_mut();
            let (on, msg) = crate::toggle_dark(cur, !cfg!(test));
            *s.dark_mut() = on;
            s.say(msg);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        zoom: f32,
        dark: bool,
        ui_scale: f32,
        said: Vec<String>,
    }
    impl Screen for Fake {
        fn zoom_mut(&mut self) -> &mut f32 {
            &mut self.zoom
        }
        fn dark_mut(&mut self) -> &mut bool {
            &mut self.dark
        }
        fn ui_scale_mut(&mut self) -> &mut f32 {
            &mut self.ui_scale
        }
        fn say(&mut self, msg: String) {
            self.said.push(msg);
        }
    }

    #[test]
    fn 拡大は2倍で止まる() {
        let mut f = Fake { zoom: 1.9, ..Default::default() };
        assert!(run(&mut f, "zoom-in"));
        assert!(run(&mut f, "zoom-in"));
        assert!((f.zoom - 2.0).abs() < 1e-6, "2倍を超えた: {}", f.zoom);
        assert!(f.said.last().unwrap().contains("200"), "{:?}", f.said);
    }

    #[test]
    fn 縮小は半分で止まる() {
        let mut f = Fake { zoom: 0.6, ..Default::default() };
        assert!(run(&mut f, "zoom-out"));
        assert!(run(&mut f, "zoom-out"));
        assert!((f.zoom - 0.5).abs() < 1e-6, "半分を下回った: {}", f.zoom);
    }

    /// 画面の明暗は押すたびに入れ替わります。**2つのアプリで同じ腕**なので、
    /// ここが1本で正しければ両方が正しくなります(2026-08-21 の B-2)
    #[test]
    fn 画面の明暗は押すたびに入れ替わる() {
        let mut f = Fake::default();
        assert!(run(&mut f, "darkmode"));
        assert!(f.dark, "1回目で暗くなる");
        assert!(run(&mut f, "darkmode"));
        assert!(!f.dark, "2回目で明るくなる");
        assert_eq!(f.said.len(), 2, "どちらも状態の行で言う: {:?}", f.said);
    }

    /// 表が使っていた古い id も受けます。rpc・MCP・Python から
    /// `theme` を送る人がいるので、黙って壊しません
    #[test]
    fn 表の古い_id_も受ける() {
        let mut f = Fake::default();
        assert!(run(&mut f, "theme"));
        assert!(f.dark);
    }

    /// 100% に戻すのは、拡大・縮小と対の命令です。**文章にしかありません
    /// でした**(2026-08-21 の B-3)。ここへ移して表にもボタンを足しました
    #[test]
    fn 百パーセントに戻す() {
        let mut f = Fake { zoom: 1.7, ..Default::default() };
        assert!(run(&mut f, "zoom100"));
        assert!((f.zoom - 1.0).abs() < 1e-6, "1.0 にならない: {}", f.zoom);
        assert!(!f.said.is_empty(), "状態の行で言う");
    }

    /// 知らない id は触らない(アプリの番)
    #[test]
    fn 知らない命令は断る() {
        let mut f = Fake { zoom: 1.0, ..Default::default() };
        assert!(!run(&mut f, "bold"));
        assert!((f.zoom - 1.0).abs() < 1e-6);
        assert!(f.said.is_empty());
    }
}
