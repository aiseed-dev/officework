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
//! {"cmd":"fill","values":{"氏名":"山田"}} → 記入欄に入れる
//! {"cmd":"open","path":"…"} / {"cmd":"save","path":"…"}
//! {"cmd":"to_pdf","path":"…"}
//! ....
//!
//! **任意のコードを走らせる動詞は置きません**(calc の口と同じ決め)。

use crate::*;

pub(crate) fn start(view: gpui::Entity<Writer>, cx: &mut gpui::App) {
    let queue: ops::Queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    if !ops::listen("writer", queue.clone()) {
        return;
    }
    // 泵: 30ms ごとに溜まった要求を主スレッドで捌く(calc と同じ刻み)
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

/// 1要求を捌く(主スレッド)。答えは JSON 1行。
pub fn handle(w: &mut Writer, line: &str) -> String {
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
                q(&w.status.to_string())
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
            let 名: Vec<String> = w.sdt_names().iter().map(|n| q(n)).collect();
            ok(&format!("\"fields\":[{}]", 名.join(",")))
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
                ops::err(&w.status.to_string())
            } else {
                ok("")
            }
        }
        "to_pdf" => {
            let Some(p) = o.str("path") else { return ops::err("path がありません") };
            w.write_pdf(std::path::Path::new(&p));
            ok("")
        }
        "end" => ok(""),
        other => ops::err(&format!("知らない動詞です: {other}")),
    }
}
