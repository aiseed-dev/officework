//! **writer の受け口**(2026-08-19 発注者「calc が rpc に対応しているので
//! あれば、writer でも使えるようにして」)。
//!
//! calc と同じ形です。`$XDG_RUNTIME_DIR/officework/writer.sock` に JSON を
//! 1行送ると、JSON を1行返します。ソケットの世話は [`ops::listen`] に1本
//! あるので、ここは*意味だけ*を持ちます。
//!
//! *表の口とは動詞が違います。* 表はセルを読み書きしますが、文書は本文と
//! 記入欄です。同じ名前の動詞(`ping` `status` `open` `save` `end`)は
//! 同じ意味にしてあります。
//!
//! ....
//! {"cmd":"ping"}            → {"ok":true,"app":"writer","version":"…"}
//! {"cmd":"status"}          → 開いている物と書きかけの有無
//! {"cmd":"text"}            → 本文(平文)
//! {"cmd":"set_text","text":"…"} → 本文を差し替える
//! {"cmd":"fields"}          → 記入欄の名前の一覧
//! {"cmd":"fill_one","name":"氏名","value":"山田"} → 差し込みの穴(`{{氏名}}`)に入れる
//! {"cmd":"fill_field","name":"氏名","value":"山田"} → 名前の付いた記入欄に入れる
//! {"cmd":"open","path":"…"} / {"cmd":"save","path":"…"}
//! {"cmd":"to_pdf","path":"…"}
//! ....
//!
//! **任意のコードを走らせる動詞は置きません**(calc の口と同じ決め)。

use crate::*;

/// ソケットを開いて要求を主スレッドへ流す。**Windows ではソケットを作らない**
/// (2026-08-20 発注者)ので、ここだけが `#[cfg(unix)]`。捌き手 [`handle`] は
/// OS を問わず組む(エージェントのパネルが直に呼ぶ)
#[cfg(unix)]
pub(crate) fn start(view: gpui::Entity<Writer>, cx: &mut gpui::App) {
    let queue: ops::Queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    if !ops::listen("writer", queue.clone()) {
        return;
    }
    // 30ms ごとに溜まった要求を主スレッドで捌く(calc と同じ刻み)
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(30))
                .await;
            let reqs: Vec<ops::Req> = std::mem::take(&mut *queue.lock().expect("受け口の錠"));
            if reqs.is_empty() {
                continue;
            }
            view.update(cx, |w, cx| {
                for req in reqs {
                    let resp = handle(w, &req.line);
                    let _ = req.reply.send(resp);
                }
                cx.notify();
            });
        }
    })
    .detach();
}

fn ok(body: &str) -> String {
    if body.is_empty() {
        "{\"ok\":true}".into()
    } else {
        format!("{{\"ok\":true,{body}}}")
    }
}

/// 字を JSON の文字列にする
fn q(s: &str) -> String {
    format!("\"{}\"", ops::jesc(s))
}

/// `from` と `to`(両端を含む)。`to` を省けば `from` だけの1つ
fn range_of(o: &ops::Jobj, n: usize) -> Result<(usize, usize), String> {
    let Some(from) = o.num("from") else { return Err("from がありません".into()) };
    if from < 0.0 {
        return Err("from は 0 以上です".into());
    }
    let from = from as usize;
    let to = o.num("to").map(|t| t as usize).unwrap_or(from);
    if n == 0 {
        return Err("文書にブロックがありません".into());
    }
    Ok((from, to))
}

/// 読んだ時の照合の字。`stamps` に "a1b2c3d4,…" と並べる(JSON を浅く保つため)
fn stamps_of(o: &ops::Jobj) -> Option<Vec<String>> {
    o.str("stamps").filter(|s| !s.trim().is_empty()).map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
}

/// 1要求を捌く(主スレッド)。答えは JSON 1行。
pub fn handle(w: &mut Writer, line: &str) -> String {
    // **受け口の要求は、それぞれが1つの操作。** 控え(checkpoint)は「この一手では
    // もう控えた」の印(acted)が立っていると取らない。印はキーやボタンの操作の
    // 始めに戻るが、受け口には戻す所が無く、前の操作の印が残ったままだと
    // 書き替えが Ctrl+Z で戻らなかった(2026-09-05、実機で見つけた)
    w.acted = false;
    let Some(o) = ops::Jobj::parse(line) else { return ops::err("JSON が読めません") };
    let Some(cmd) = o.str("cmd") else { return ops::err("cmd がありません") };
    match cmd.as_str() {
        "ping" => ok(&format!("\"app\":\"writer\",\"version\":{}", q(env!("CARGO_PKG_VERSION")))),
        "status" => {
            let path = w.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            ok(&format!(
                "\"path\":{},\"dirty\":{},\"docs\":{},\"doc_at\":{},\"status\":{}",
                q(&path),
                w.dirty,
                w.doc_count(),
                w.doc_at,
                q(&w.status)
            ))
        }
        // **本文を読む。** いま見ている文書の分だけ(何枚目かは status)
        "text" => ok(&format!("\"text\":{}", q(&w.doc.body_text()))),
        "set_text" => {
            let Some(t) = o.str("text") else { return ops::err("text がありません") };
            w.checkpoint(false);
            w.doc.set_body_text(&t);
            w.ed = Editor::new(&w.doc.body_text());
            w.dirty = true;
            w.lay();
            ok("")
        }
        // 記入欄(様式の穴)の名前
        "fields" => {
            let name: Vec<String> = w.sdt_names().iter().map(|n| q(n)).collect();
            ok(&format!("\"fields\":[{}]", name.join(",")))
        }
        // **差し込みの穴(`{{member}}`)の名前**(2026-08-21 の C-2)。
        //
        // `fields` とは別の仕組みです。あちらは文書に埋め込んだ記入欄
        // (Word の入力コントロール)で、こちらは本文に書いた `{{member}}` です。
        // 道具の側で「その名前があるか」を先に見られないと、`fill_one` の
        // 結果から可否が読めません(`unknown` は*渡さなかった残りの穴*で、
        // 渡した名前が入ったかどうかではないため)。
        //
        // **文書は変えません。** 空のデータで通して、埋まらなかった名前を
        // 集めるだけです。埋まらない穴はそのまま残る作りなので、これで
        // 全部の名前が出ます。返ってきた文書は捨てます
        "merge_fields" => {
            let (_, rep) = kumihan::fill::fill(&w.doc, &kumihan::fill::Data::new());
            let name: Vec<String> = rep.unknown.iter().map(|n| q(n)).collect();
            ok(&format!("\"merge_fields\":[{}]", name.join(",")))
        }
        // **名前の付いた記入欄(w:sdt)に入れる。** `fill_one` は本文の
        // `{{名前}}` の穴で、こちらは Word の入力コントロールです。
        // 返りは書いた欄の数。0 ならその名前の欄が無い
        "fill_field" => {
            let (Some(name), Some(value)) = (o.str("name"), o.str("value")) else {
                return ops::err("name と value が要ります");
            };
            w.checkpoint(false);
            let n = w.doc.set_sdt_text(&name, &value);
            if n == 0 {
                return ops::err(&format!("記入欄「{name}」が見つかりません"));
            }
            w.ed = Editor::new(&w.doc.body_text());
            w.dirty = true;
            w.lay();
            ok(&format!("\"filled\":{n}"))
        }
        // **1つの記入欄に入れる。** まとめて入れる形は、値の並びを浅い
        // JSON で運べないので、呼ぶ側が繰り返します(道具の側で回す)
        "fill_one" => {
            let (Some(name), Some(value)) = (o.str("name"), o.str("value")) else {
                return ops::err("name と value が要ります");
            };
            let mut d = kumihan::fill::Data::new();
            d.set(&name, value);
            w.checkpoint(false);
            let (doc, rep) = kumihan::fill::fill(&w.doc, &d);
            w.doc = doc;
            w.ed = Editor::new(&w.doc.body_text());
            w.dirty = true;
            w.lay();
            ok(&format!("\"unknown\":{},\"summary\":{}", rep.unknown.len(), q(&rep.summary())))
        }
        "open" => {
            let Some(p) = o.str("path") else { return ops::err("path がありません") };
            if w.dirty {
                return ops::err("書きかけがあります(先に保存してください)");
            }
            w.open(PathBuf::from(p));
            ok("")
        }
        "save" => {
            let p = o.str("path").map(PathBuf::from).or_else(|| w.path.clone());
            let Some(p) = p else { return ops::err("保存先がありません") };
            w.save_to(p);
            if w.dirty {
                ops::err(&w.status)
            } else {
                ok("")
            }
        }
        "to_pdf" => {
            let Some(p) = o.str("path") else { return ops::err("path がありません") };
            w.write_pdf(std::path::Path::new(&p));
            ok("")
        }
        // ── ブロックの語彙(2026-09-04。docs/sekkei/agent.ja.adoc「writer にも同じパネル」)──
        // 本文の丸ごとではなく、ブロックの番号で AsciiDoc の字を読み書きする。
        // エージェント・Python・MCP が同じ名前で呼ぶ。実体は kumihan::blocks
        "outline" => {
            let items: Vec<String> = kumihan::blocks::outline(&w.doc)
                .iter()
                .map(|h| format!("{{\"index\":{},\"level\":{},\"text\":{}}}", h.index, h.level, q(&h.text)))
                .collect();
            ok(&format!("\"count\":{},\"outline\":[{}]", w.doc.blocks.len(), items.join(",")))
        }
        "read_blocks" => {
            let (from, to) = match range_of(&o, w.doc.blocks.len()) {
                Ok(r) => r,
                Err(e) => return ops::err(&e),
            };
            match kumihan::blocks::read(&w.doc, from, to) {
                Ok(v) => {
                    let items: Vec<String> = v
                        .iter()
                        .map(|b| format!("{{\"index\":{},\"stamp\":{},\"adoc\":{}}}", b.index, q(&b.stamp), q(&b.adoc)))
                        .collect();
                    ok(&format!("\"blocks\":[{}]", items.join(",")))
                }
                Err(e) => ops::err(&e),
            }
        }
        "replace_blocks" => {
            let (from, to) = match range_of(&o, w.doc.blocks.len()) {
                Ok(r) => r,
                Err(e) => return ops::err(&e),
            };
            let Some(src) = o.str("adoc") else { return ops::err("adoc がありません") };
            let stamps = stamps_of(&o);
            one_step(w, |w| {
                kumihan::blocks::replace(&mut w.doc, from, to, &src, stamps.as_deref()).map(|n| format!("\"replaced\":{n}"))
            })
        }
        "insert_blocks" => {
            let Some(at) = o.num("at") else { return ops::err("at がありません") };
            let Some(src) = o.str("adoc") else { return ops::err("adoc がありません") };
            one_step(w, |w| kumihan::blocks::insert(&mut w.doc, at as usize, &src).map(|n| format!("\"inserted\":{n}")))
        }
        "delete_blocks" => {
            let (from, to) = match range_of(&o, w.doc.blocks.len()) {
                Ok(r) => r,
                Err(e) => return ops::err(&e),
            };
            let stamps = stamps_of(&o);
            one_step(w, |w| kumihan::blocks::delete(&mut w.doc, from, to, stamps.as_deref()).map(|n| format!("\"deleted\":{n}")))
        }
        "find" => {
            let Some(text) = o.str("text") else { return ops::err("text がありません") };
            let hits: Vec<String> = kumihan::blocks::find(&w.doc, &text)
                .iter()
                .map(|(i, around)| format!("{{\"index\":{i},\"around\":{}}}", q(around)))
                .collect();
            ok(&format!("\"hits\":[{}]", hits.join(",")))
        }
        // 記入欄をまとめて入れる。values は [[名前, 値], …]。無い名前は missing に返す
        "fill_fields" => {
            let Some(rows) = o.grid("values") else { return ops::err("values がありません([[名前, 値], …])") };
            let mut pairs: Vec<(String, String)> = Vec::new();
            for r in &rows {
                match (r.first(), r.get(1)) {
                    (Some(ops::J::S(n)), Some(v)) => {
                        let v = match v {
                            ops::J::S(s) => s.clone(),
                            other => other.to_json().trim_matches('"').to_string(),
                        };
                        pairs.push((n.clone(), v));
                    }
                    _ => return ops::err("values の各行は [名前, 値] です"),
                }
            }
            one_step(w, |w| {
                let mut filled = 0usize;
                let mut missing: Vec<String> = Vec::new();
                for (n, v) in &pairs {
                    let k = w.doc.set_sdt_text(n, v);
                    if k == 0 {
                        missing.push(q(n));
                    } else {
                        filled += k;
                    }
                }
                Ok(format!("\"filled\":{filled},\"missing\":[{}]", missing.join(",")))
            })
        }
        // ── マクロ(パネルから起こした officework-mcp の run_macro。2026-09-05)──
        // 始めて番号を返し、様子は別の動詞で見る(受け口を止めない)。形は
        // 表の受け口(ops)と同じ
        "macro_start" => {
            let Some(code) = o.str("code") else { return ops::err("code がありません") };
            let name = o.str("name").unwrap_or_else(|| "agent_macro".into());
            match w.macro_start_job(&code, &name) {
                Ok(id) => ok(&format!("\"id\":{id}")),
                Err(e) => ops::err(&e),
            }
        }
        "macro_status" => {
            let Some(id) = o.num("id") else { return ops::err("id がありません") };
            let (state, text) = match w.macro_poll(id as u64) {
                ops::MacroStatus::Running => ("running", String::new()),
                ops::MacroStatus::Done(t) => ("done", t),
                ops::MacroStatus::Failed(t) => ("failed", t),
                ops::MacroStatus::Unknown => return ops::err("その番号のマクロはありません"),
            };
            ok(&format!("\"state\":\"{state}\",\"text\":{}", q(&text)))
        }
        "end" => ok(""),
        other => ops::err(&format!("知らない動詞です: {other}")),
    }
}

/// **書き替えの1手。** 控えを取ってから直し、断ったら控えを捨てる(断った
/// 要求の控えが残ると、次の Ctrl+Z がそれを戻して見た目が変わらない)
fn one_step(w: &mut Writer, f: impl FnOnce(&mut Writer) -> Result<String, String>) -> String {
    w.checkpoint(false);
    match f(w) {
        Ok(body) => {
            w.after_block_edit();
            ok(&body)
        }
        Err(e) => {
            w.undo_stack.pop();
            w.acted = false;
            ops::err(&e)
        }
    }
}

impl Writer {
    /// ブロックの語彙で文書を書き替えた後の整え(画面の字・書きかけの印・組み直し)
    pub(crate) fn after_block_edit(&mut self) {
        self.ed = Editor::new(&self.doc.body_text());
        self.dirty = true;
        self.lay();
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    /// **マクロの直した字は1手として入り、Ctrl+Z で戻る**(受け口とパネルの
    /// 両方が通る `macro_apply`)。サンドボックスを使わずに、直した字を手で当てる
    #[gpui::test]
    fn a_document_macro_result_lands_as_one_undo_step(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            let (d, _) = kumihan::adoc::parse_full("= 報告\n\n== 概況\n\n受注は3件。\n").unwrap();
            this.set_doc(d);
            let before = this.undo_stack.len();
            let r = this.macro_apply("= 報告\n\n== 概況(改)\n\n受注は4件。\n", String::new()).unwrap();
            assert_eq!(r, "終わりました");
            assert!(this.doc.body_text().contains("受注は4件"), "本文が替わらない: {}", this.doc.body_text());
            assert_eq!(this.undo_stack.len(), before + 1, "控えは1回だけ");
            this.undo_step();
            assert!(this.doc.body_text().contains("受注は3件"), "Ctrl+Z で戻らない: {}", this.doc.body_text());
            // 読めない字は断る(文書は変わらない)
            assert!(this.macro_apply("|===\n|閉じない表\n", String::new()).is_err());
            assert!(this.doc.body_text().contains("受注は3件"));
        });
    }

    /// **受け口のマクロは、始めて番号を返し、様子は別の動詞で見る**(表の
    /// 受け口と同じ形。2026-09-05)。断り方と番号の作法だけを固定する
    #[gpui::test]
    fn the_rpc_starts_a_macro_and_reports_its_state_by_number(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            let r = crate::rpc::handle(this, r#"{"cmd":"macro_start"}"#);
            assert!(r.contains("code"), "code 無しを断らない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"macro_status","id":99}"#);
            assert!(r.contains("\"err\"") && r.contains("番号"), "知らない番号を断らない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"macro_start","code":"out = src"}"#);
            if r.contains("\"id\":1") {
                let r = crate::rpc::handle(this, r#"{"cmd":"macro_status","id":1}"#);
                assert!(r.contains("\"state\":\""), "様子が返らない: {r}");
            } else {
                assert!(r.contains("\"err\""), "始まりも断りもしない: {r}");
            }
        });
    }

    #[cfg(unix)]
    #[gpui::test]
    fn the_rpc_reads_and_rewrites_blocks_by_number(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.native = false;
            let d = kumihan::adoc::parse("= 報告\n\n== 概況\n\n受注は3件。\n\n== 予定\n\n8月に着手。\n").unwrap();
            this.set_doc(d);
            let r = crate::rpc::handle(this, r#"{"cmd":"outline"}"#);
            assert!(r.contains("\"count\":5") && r.contains("\"text\":\"予定\""), "地図が出ない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"read_blocks","from":2}"#);
            assert!(r.contains("\"adoc\":\"受注は3件。\\n\""), "ブロックが読めない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"replace_blocks","from":2,"to":2,"adoc":"受注は4件。\n\n* 外壁\n"}"#);
            assert!(r.contains("\"replaced\":2"), "書き替えられない: {r}");
            assert!(this.doc.body_text().contains("受注は4件"), "本文に無い: {}", this.doc.body_text());
            assert!(this.ed.text().contains("受注は4件"), "画面の字に無い");
            assert!(this.dirty);
            let r = crate::rpc::handle(this, r#"{"cmd":"find","text":"外壁"}"#);
            assert!(r.contains("\"index\":3"), "探せない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"delete_blocks","from":3,"stamps":"00000000"}"#);
            assert!(r.contains("変わっています"), "古い照合の字を断らない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"insert_blocks","at":99,"adoc":"x\n"}"#);
            assert!(r.contains("範囲の外"), "範囲の外を断らない: {r}");
            // Ctrl+Z 1手で戻る
            this.undo_step();
            assert!(!this.doc.body_text().contains("受注は4件"), "1手で戻らない");
            // 前の操作の印(acted)が残っていても、受け口の要求は自分の一手として控える
            this.acted = true;
            let r = crate::rpc::handle(this, r#"{"cmd":"replace_blocks","from":2,"to":2,"adoc":"受注は9件。\n"}"#);
            assert!(r.contains("\"replaced\":1"), "{r}");
            this.undo_step();
            assert!(!this.doc.body_text().contains("受注は9件"), "印が残っていると1手で戻らない");
        });
    }
}
