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
            let msg = crate::tf!("ズーム {}%", pct);
            s.say(msg.to_string());
            true
        }
        // AI の宛先を順に替える。**writer と calc で同じ言い回し**
        "ai-where" => {
            let next = crate::ai::backend().next();
            crate::ai::set_backend(next);
            let msg = match crate::ai::ready(next) {
                Ok(_) => crate::tf!("AI の宛先: {}(覚えました)", next.label()),
                Err(e) => {
                    crate::tf!("AI の宛先: {} — ただし今は使えません: {}", next.label(), e)
                }
            };
            s.say(msg.to_string());
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        zoom: f32,
        said: Vec<String>,
    }
    impl Screen for Fake {
        fn zoom_mut(&mut self) -> &mut f32 {
            &mut self.zoom
        }
        fn say(&mut self, msg: String) {
            self.said.push(msg);
        }
    }

    #[test]
    fn 拡大は2倍で止まる() {
        let mut f = Fake { zoom: 1.9, said: vec![] };
        assert!(run(&mut f, "zoom-in"));
        assert!(run(&mut f, "zoom-in"));
        assert!((f.zoom - 2.0).abs() < 1e-6, "2倍を超えた: {}", f.zoom);
        assert!(f.said.last().unwrap().contains("200"), "{:?}", f.said);
    }

    #[test]
    fn 縮小は半分で止まる() {
        let mut f = Fake { zoom: 0.6, said: vec![] };
        assert!(run(&mut f, "zoom-out"));
        assert!(run(&mut f, "zoom-out"));
        assert!((f.zoom - 0.5).abs() < 1e-6, "半分を下回った: {}", f.zoom);
    }

    /// 知らない id は触らない(アプリの番)
    #[test]
    fn 知らない命令は断る() {
        let mut f = Fake { zoom: 1.0, said: vec![] };
        assert!(!run(&mut f, "bold"));
        assert!((f.zoom - 1.0).abs() < 1e-6);
        assert!(f.said.is_empty());
    }
}
