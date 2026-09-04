//! **エージェントパネルのループの骨。** oh-my-pi(MIT)の設計の蒸留です。
//!
//! 写したのは形だけ — モデルに道具の一覧を渡し、返ってきた道具呼びを
//! 実行し、結果を渡して続きをもらう。道具呼びが無くなったら、それが
//! 1つの答えです(docs/sekkei/agent.ja.adoc)。
//!
//! このクレートは道具の実体を持ちません。道具は [`ToolHost`] の trait で
//! 受け取り、アプリの側が操作の語彙(ops)への結び付けを渡します。
//! モデルも [`Model`] の trait で受け取るので、決まった応答を返す
//! 偽のモデルで往復を試験できます(実物は [`EndpointModel`])。

use lang::model::{chat_tools, ChatOut, Endpoint, Msg, ToolCall, ToolDef};

#[cfg(feature = "ops")]
pub mod tools;

/// 宛先「Claude Code」: 改変していない `claude` を子プロセスで(定額の道)
pub mod claude_code;

/// モデルへの1往復。実物(宛先)も試験の偽物も同じ面を持つ
pub trait Model {
    fn chat(&mut self, msgs: &[Msg], tools: &[ToolDef]) -> Result<ChatOut, String>;
}

/// lang::model の宛先で話す実物。提供元の切り替えは Endpoint の
/// 差し替えそのもの — 手元のモデルもクラウドも同じ扱い(決め 985aa858)
pub struct EndpointModel {
    pub ep: Endpoint,
    pub temperature: f32,
}

impl Model for EndpointModel {
    fn chat(&mut self, msgs: &[Msg], tools: &[ToolDef]) -> Result<ChatOut, String> {
        chat_tools(&self.ep, msgs, tools, self.temperature)
    }
}

/// 道具の結線。アプリの側が実装して渡す(calc の Host と同じ形)。
/// エージェント専用の隠れた道具は作らない — 道具を増やすときは
/// まず ops の語彙に足す(設計の決め)
pub trait ToolHost {
    /// モデルに渡す道具の一覧
    fn tools(&self) -> Vec<ToolDef>;
    /// 道具を1つ実行して、モデルへ返す字を作る。
    /// しくじりも Err の字で返す — ループが「エラー:」を付けてモデルに
    /// 渡し、直させる(黙って止めない)
    fn call(&mut self, name: &str, arguments: &str) -> Result<String, String>;
}

/// 対話の記録の1行。画面の1行表示と、セッションの保存の材料
/// (保存の形は段階4で決める)
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    User(String),
    Assistant(String),
    ToolCall { name: String, arguments: String },
    ToolResult { name: String, content: String, ok: bool },
}

/// 1つの対話。履歴(モデルに送る形)と記録(人が読む形)を持つ
pub struct Agent {
    msgs: Vec<Msg>,
    pub log: Vec<Event>,
    /// 1つの頼みで道具を呼べる上限。守りの数字で、普通は届かない。
    /// 届いたら Err で止める — 同じ呼びを繰り返すモデルを回し続けない
    pub max_calls: usize,
    /// いまの頼みで道具を呼んだ数(begin で 0 に戻る)
    used: usize,
}

impl Agent {
    pub fn new(system: &str) -> Self {
        Agent { msgs: vec![Msg::System(system.into())], log: Vec::new(), max_calls: 25, used: 0 }
    }

    /// 頼みを1つ積む。返りはモデルへ送る履歴。
    ///
    /// 画面のあるアプリは begin と [`Agent::feed`] で1往復ずつ進める —
    /// モデルの往復は裏の糸で待ち、道具はメインスレッドで実行するため。
    /// 全部を1息に回してよい所(試験・画面の無い口)は [`Agent::ask`]
    pub fn begin(&mut self, user: &str) -> Vec<Msg> {
        self.msgs.push(Msg::User(user.into()));
        self.log.push(Event::User(user.into()));
        self.used = 0;
        self.msgs.clone()
    }

    /// モデルの答えを1つ受け取り、道具呼びがあれば実行する。
    /// 答えが出たら Some(答えの字)。まだなら None — [`Agent::msgs`] を
    /// 添えてもう一度 chat する
    pub fn feed(
        &mut self,
        out: ChatOut,
        host: &mut dyn ToolHost,
    ) -> Result<Option<String>, String> {
        if out.tool_calls.is_empty() {
            self.finish(&out.content);
            return Ok(Some(out.content));
        }
        self.note_calls(&out.tool_calls);
        for c in &out.tool_calls {
            self.count_call()?;
            self.note_call(c);
            let r = host.call(&c.name, &c.arguments);
            self.tool_result(c, r);
        }
        Ok(None)
    }

    // ---- feed のばら売り ----
    //
    // 実行が裏の糸に跨る道具(マクロ・保存の確認)を挟むアプリは、
    // feed を使わずこの4つで1つずつ進める。順は
    // note_calls → (count_call → note_call → 実行 → tool_result)×呼びの数 →
    // 全部済んだら次の chat、呼びの無い答えが来たら finish

    /// 道具呼びの番を履歴に積む(次の往復でモデルに送り返す)
    pub fn note_calls(&mut self, calls: &[ToolCall]) {
        self.msgs.push(Msg::AssistantCalls(calls.to_vec()));
    }

    /// 道具を1つ数える。上限を超えたら Err(打ち切りの文)
    pub fn count_call(&mut self) -> Result<(), String> {
        self.used += 1;
        if self.used > self.max_calls {
            Err(format!("道具を {} 回呼んでも終わりません — 打ち切りました", self.max_calls))
        } else {
            Ok(())
        }
    }

    /// 呼びの記録(**実行の前に**出す — 黙って動かさない)
    pub fn note_call(&mut self, c: &ToolCall) {
        self.log.push(Event::ToolCall { name: c.name.clone(), arguments: c.arguments.clone() });
    }

    /// 道具の結果を1つ入れる。しくじりは「エラー:」を頭に付けて
    /// モデルに渡し、直させる(黙って止めない)
    pub fn tool_result(&mut self, c: &ToolCall, r: Result<String, String>) {
        let (content, ok) = match r {
            Ok(s) => (s, true),
            Err(e) => (format!("エラー: {e}"), false),
        };
        self.log.push(Event::ToolResult { name: c.name.clone(), content: content.clone(), ok });
        self.msgs.push(Msg::ToolResult { id: c.id.clone(), content });
    }

    /// 答えで締める(呼びの無い答えが来たとき)
    pub fn finish(&mut self, content: &str) {
        self.msgs.push(Msg::Assistant(content.into()));
        self.log.push(Event::Assistant(content.into()));
    }

    /// いまの履歴(モデルへ送る形)。feed の続きの chat が使う
    pub fn msgs(&self) -> Vec<Msg> {
        self.msgs.clone()
    }

    /// 1つの頼みを最後まで回す。道具呼びが無くなったら答えの字を返す
    pub fn ask(
        &mut self,
        model: &mut dyn Model,
        host: &mut dyn ToolHost,
        user: &str,
    ) -> Result<String, String> {
        let mut msgs = self.begin(user);
        let tools = host.tools();
        loop {
            let out = model.chat(&msgs, &tools)?;
            match self.feed(out, host)? {
                Some(answer) => return Ok(answer),
                None => msgs = self.msgs(),
            }
        }
    }

    /// これまでの発言の数(試験と画面の目安)
    pub fn len(&self) -> usize {
        self.msgs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.msgs.len() <= 1 // system だけなら空
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lang::model::ToolCall;

    /// 決まった応答を順に返す偽のモデル
    struct Fake {
        outs: Vec<ChatOut>,
        /// 受け取った履歴の長さの控え(往復のたびに増えることを見る)
        seen: Vec<usize>,
    }

    impl Model for Fake {
        fn chat(&mut self, msgs: &[Msg], _tools: &[ToolDef]) -> Result<ChatOut, String> {
            self.seen.push(msgs.len());
            if self.outs.is_empty() {
                return Err("応答が尽きました".into());
            }
            Ok(self.outs.remove(0))
        }
    }

    /// 範囲の読みだけを持つ偽の結線(段階1の道具1つ)
    struct Grid;

    impl ToolHost for Grid {
        fn tools(&self) -> Vec<ToolDef> {
            vec![ToolDef {
                name: "read_range".into(),
                description: "セル範囲の値を読む".into(),
                parameters:
                    r#"{"type":"object","properties":{"a1":{"type":"string"}},"required":["a1"]}"#
                        .into(),
            }]
        }
        fn call(&mut self, name: &str, arguments: &str) -> Result<String, String> {
            if name != "read_range" {
                return Err(format!("知らない道具: {name}"));
            }
            if arguments.contains("A1:B2") {
                Ok("1\t2\n3\t4".into())
            } else {
                Err("その範囲は読めません".into())
            }
        }
    }

    fn call(name: &str, args: &str) -> ChatOut {
        ChatOut {
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: name.into(),
                arguments: args.into(),
            }],
            ..Default::default()
        }
    }

    fn answer(s: &str) -> ChatOut {
        ChatOut { content: s.into(), ..Default::default() }
    }

    #[test]
    fn a_tool_call_round_trip_reaches_the_answer() {
        let mut model = Fake {
            outs: vec![call("read_range", r#"{"a1":"A1:B2"}"#), answer("合計は 10 です")],
            seen: Vec::new(),
        };
        let mut agent = Agent::new("あなたは表の助手です");
        let ans = agent.ask(&mut model, &mut Grid, "A1:B2 の合計は?").unwrap();
        assert_eq!(ans, "合計は 10 です");
        // 2往復目は「呼び+結果」のぶん履歴が伸びている
        assert_eq!(model.seen, vec![2, 4]);
        assert_eq!(
            agent.log,
            vec![
                Event::User("A1:B2 の合計は?".into()),
                Event::ToolCall {
                    name: "read_range".into(),
                    arguments: r#"{"a1":"A1:B2"}"#.into()
                },
                Event::ToolResult {
                    name: "read_range".into(),
                    content: "1\t2\n3\t4".into(),
                    ok: true
                },
                Event::Assistant("合計は 10 です".into()),
            ]
        );
    }

    #[test]
    fn a_tool_error_goes_back_to_the_model() {
        // しくじりで止めず、「エラー:」の字を結果としてモデルに返す
        let mut model = Fake {
            outs: vec![
                call("read_range", r#"{"a1":"Z99"}"#),
                answer("その範囲は読めませんでした"),
            ],
            seen: Vec::new(),
        };
        let mut agent = Agent::new("s");
        let ans = agent.ask(&mut model, &mut Grid, "Z99 を読んで").unwrap();
        assert_eq!(ans, "その範囲は読めませんでした");
        assert!(agent.log.iter().any(|e| matches!(
            e,
            Event::ToolResult { ok: false, content, .. } if content.starts_with("エラー:")
        )));
    }

    #[test]
    fn a_runaway_loop_is_cut_off() {
        // 同じ呼びを返し続けるモデルを、上限で打ち切る
        let outs: Vec<ChatOut> =
            (0..30).map(|_| call("read_range", r#"{"a1":"A1:B2"}"#)).collect();
        let mut model = Fake { outs, seen: Vec::new() };
        let mut agent = Agent::new("s");
        agent.max_calls = 3;
        let e = agent.ask(&mut model, &mut Grid, "u").unwrap_err();
        assert!(e.contains("打ち切りました"), "{e}");
    }

    #[test]
    fn the_second_ask_keeps_the_first_conversation() {
        // 対話は続き物 — 2つ目の頼みは1つ目の履歴の上に載る
        let mut model = Fake {
            outs: vec![answer("はい"), answer("2つ目です")],
            seen: Vec::new(),
        };
        let mut agent = Agent::new("s");
        agent.ask(&mut model, &mut Grid, "1つ目").unwrap();
        agent.ask(&mut model, &mut Grid, "2つ目").unwrap();
        assert_eq!(model.seen, vec![2, 4], "履歴が積み上がっていない");
        assert!(!agent.is_empty());
    }
}
