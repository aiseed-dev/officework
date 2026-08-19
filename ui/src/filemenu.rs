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
