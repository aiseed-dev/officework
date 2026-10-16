//! **The seven kinds of message** (`MessageAction` on the Python side; the
//! numbers must match the Dart enum of the same name).
//!
//! A message is the array `[action_code, body]`. The bodies are maps with
//! the field names the Python dataclasses use (`flet/messaging/protocol.py`).

use crate::msgpack::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The client's registration request, and Python's response
    RegisterClient = 1,
    /// Python → client: a patch to a control's tree
    PatchControl = 2,
    /// Client → Python: an event of a control
    ControlEvent = 3,
    /// Client → Python: properties the client changed (an input's value)
    UpdateControlProps = 4,
    /// Python → client: call a control's method; client → Python: its result
    InvokeMethod = 5,
    /// Python → client: the session failed
    SessionCrashed = 6,
    /// Pyodide only: a line of Python's stdout or stderr
    PythonOutput = 7,
}

impl Action {
    pub fn from_code(n: i64) -> Option<Action> {
        Some(match n {
            1 => Action::RegisterClient,
            2 => Action::PatchControl,
            3 => Action::ControlEvent,
            4 => Action::UpdateControlProps,
            5 => Action::InvokeMethod,
            6 => Action::SessionCrashed,
            7 => Action::PythonOutput,
            _ => return None,
        })
    }

    pub fn code(self) -> i64 {
        self as i64
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub action: Action,
    pub body: Value,
}

impl Message {
    pub fn new(action: Action, body: Value) -> Message {
        Message { action, body }
    }

    pub fn to_value(&self) -> Value {
        Value::Array(vec![Value::Int(self.action.code()), self.body.clone()])
    }

    pub fn from_value(v: Value) -> Result<Message, String> {
        let Value::Array(mut a) = v else { return Err("メッセージは配列です".into()) };
        if a.len() != 2 {
            return Err(format!("メッセージは [action, body] の 2 つです({} 個)", a.len()));
        }
        let body = a.pop().expect("2 items");
        let code = a.pop().expect("2 items").as_i64().ok_or("action が数ではありません")?;
        let action = Action::from_code(code).ok_or_else(|| format!("知らない action {code}"))?;
        Ok(Message { action, body })
    }
}

fn opt_str(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str()).map(str::to_string)
}

fn need_i64(v: &Value, key: &str) -> Result<i64, String> {
    v.get(key).and_then(Value::as_i64).ok_or_else(|| format!("{key} がありません"))
}

fn need_str(v: &Value, key: &str) -> Result<String, String> {
    opt_str(v.get(key)).ok_or_else(|| format!("{key} がありません"))
}

/// The client's registration: `session_id` is None the first time, the old
/// id when reconnecting; `page` is a patch of the Page's properties
/// (`client::page_props` makes the usual one)
#[derive(Clone, Debug)]
pub struct RegisterRequest {
    pub session_id: Option<String>,
    pub page_name: String,
    pub page: Value,
}

impl RegisterRequest {
    pub fn to_value(&self) -> Value {
        Value::map(vec![
            ("session_id", self.session_id.as_deref().map(Value::str).unwrap_or(Value::Nil)),
            ("page_name", Value::str(&self.page_name)),
            ("page", self.page.clone()),
        ])
    }
}

/// Python's answer: the session id and the whole Page tree
#[derive(Clone, Debug)]
pub struct RegisterResponse {
    pub session_id: Option<String>,
    pub page_patch: Value,
    pub error: Option<String>,
}

impl RegisterResponse {
    pub fn from_value(v: &Value) -> Result<RegisterResponse, String> {
        Ok(RegisterResponse {
            session_id: opt_str(v.get("session_id")),
            page_patch: v.get("page_patch").cloned().unwrap_or(Value::Nil),
            error: opt_str(v.get("error")).filter(|e| !e.is_empty()),
        })
    }
}

/// A patch to apply with the control `id` as the root
#[derive(Clone, Debug)]
pub struct PatchControl {
    pub id: i64,
    pub patch: Value,
}

impl PatchControl {
    pub fn from_value(v: &Value) -> Result<PatchControl, String> {
        Ok(PatchControl { id: need_i64(v, "id")?, patch: v.get("patch").cloned().unwrap_or(Value::Nil) })
    }
}

/// An event of a control: `name` without the `on_`, `data` as the event defines
#[derive(Clone, Debug)]
pub struct ControlEvent {
    pub target: i64,
    pub name: String,
    pub data: Value,
}

impl ControlEvent {
    pub fn to_value(&self) -> Value {
        Value::map(vec![
            ("target", Value::Int(self.target)),
            ("name", Value::str(&self.name)),
            ("data", self.data.clone()),
        ])
    }
}

/// Properties the client changed, written back to the Python control
#[derive(Clone, Debug)]
pub struct UpdateProps {
    pub id: i64,
    pub props: Value,
}

impl UpdateProps {
    pub fn to_value(&self) -> Value {
        Value::map(vec![("id", Value::Int(self.id)), ("props", self.props.clone())])
    }
}

/// Python asks the client to run a control's method
#[derive(Clone, Debug)]
pub struct InvokeRequest {
    pub control_id: i64,
    pub call_id: String,
    pub name: String,
    pub args: Value,
    pub timeout: Value,
}

impl InvokeRequest {
    pub fn from_value(v: &Value) -> Result<InvokeRequest, String> {
        Ok(InvokeRequest {
            control_id: need_i64(v, "control_id")?,
            call_id: need_str(v, "call_id")?,
            name: need_str(v, "name")?,
            args: v.get("args").cloned().unwrap_or(Value::Nil),
            timeout: v.get("timeout").cloned().unwrap_or(Value::Nil),
        })
    }
}

/// The client's answer to an [`InvokeRequest`]
#[derive(Clone, Debug)]
pub struct InvokeResponse {
    pub control_id: i64,
    pub call_id: String,
    pub result: Value,
    pub error: Option<String>,
}

impl InvokeResponse {
    pub fn to_value(&self) -> Value {
        Value::map(vec![
            ("control_id", Value::Int(self.control_id)),
            ("call_id", Value::str(&self.call_id)),
            ("result", self.result.clone()),
            ("error", self.error.as_deref().map(Value::str).unwrap_or(Value::Nil)),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct SessionCrashed {
    pub message: String,
}

impl SessionCrashed {
    pub fn from_value(v: &Value) -> SessionCrashed {
        SessionCrashed { message: opt_str(v.get("message")).unwrap_or_default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_an_array_of_code_and_body() {
        let m = Message::new(
            Action::ControlEvent,
            ControlEvent { target: 10, name: "click".into(), data: Value::Nil }.to_value(),
        );
        let v = m.to_value();
        assert_eq!(v.as_array().unwrap()[0], Value::Int(3));
        assert_eq!(Message::from_value(v).unwrap(), m);
        assert!(Message::from_value(Value::Array(vec![Value::Int(9), Value::Nil])).is_err());
    }

    #[test]
    fn the_registration_response_is_read_as_python_sends_it() {
        let v = Value::map(vec![
            ("session_id", Value::str("NPs2SUizqowJh19Y")),
            ("page_patch", Value::map(vec![("_c", Value::str("Page")), ("_i", Value::Int(1))])),
            ("error", Value::str("")),
        ]);
        let r = RegisterResponse::from_value(&v).unwrap();
        assert_eq!(r.session_id.as_deref(), Some("NPs2SUizqowJh19Y"));
        assert_eq!(r.error, None);
        assert_eq!(r.page_patch.get("_c").and_then(Value::as_str), Some("Page"));
    }
}
