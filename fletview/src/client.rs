//! **A client on a socket**: connects to the page URL a Flet program hands
//! over (`flet <url> <pid file>`), registers, keeps the tree up to date from
//! the patches, and sends events, property updates and method results.
//!
//! The calls block on the socket, so the gpui side runs them on a thread of
//! their own and hands the results over (the tree is plain data).

use std::io::{self, Read, Write};
use std::net::TcpStream;

use crate::message::{
    Action, ControlEvent, InvokeRequest, InvokeResponse, Message, PatchControl, RegisterRequest, RegisterResponse,
    SessionCrashed, UpdateProps,
};
use crate::msgpack::Value;
use crate::tree::{ControlId, Tree};
use crate::wire::{read_packet, write_message, Packet};

enum Stream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Stream::Unix(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Tcp(s) => s.write(buf),
            #[cfg(unix)]
            Stream::Unix(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Tcp(s) => s.flush(),
            #[cfg(unix)]
            Stream::Unix(s) => s.flush(),
        }
    }
}

/// What arrived from Python
#[derive(Debug)]
pub enum Incoming {
    /// A patch was applied; these controls changed and are drawn again
    Patched { root: ControlId, changed: Vec<ControlId> },
    /// Python asks for a control's method; answer with [`Client::invoke_result`]
    Invoke(InvokeRequest),
    Crashed(String),
    /// A line of Python's output (Pyodide only)
    Output { text: String, is_stderr: bool },
    /// A data channel frame
    Data { channel: u32, bytes: Vec<u8> },
    /// A message this client has no use for
    Other(Message),
}

pub struct Client {
    stream: Stream,
    pub tree: Tree,
    pub session_id: Option<String>,
}

/// The Page properties the Dart client sends when it registers: the ones
/// Python's Page needs before `main` runs (`flet_backend.dart`,
/// `_registerClient`)
pub fn page_props(width: f64, height: f64, platform: &str) -> Value {
    let zero = || Value::map(vec![("top", Value::Int(0)), ("right", Value::Int(0)), ("bottom", Value::Int(0)), ("left", Value::Int(0))]);
    Value::map(vec![
        ("route", Value::str("/")),
        ("pwa", Value::Bool(false)),
        ("web", Value::Bool(false)),
        ("debug", Value::Bool(false)),
        ("wasm", Value::Bool(false)),
        ("test", Value::Bool(false)),
        ("multi_view", Value::Bool(false)),
        ("pyodide", Value::Bool(false)),
        ("platform_brightness", Value::str("light")),
        ("width", Value::F64(width)),
        ("height", Value::F64(height)),
        ("platform", Value::str(platform)),
        (
            "window",
            Value::map(vec![("_c", Value::str("Window")), ("_i", Value::Int(0)), ("width", Value::F64(width)), ("height", Value::F64(height))]),
        ),
        ("media", Value::map(vec![("padding", zero()), ("view_padding", zero()), ("view_insets", zero())])),
    ])
}

impl Client {
    /// Connects to `tcp://host:port` or a Unix socket path
    pub fn connect(url: &str) -> io::Result<Client> {
        let stream = if let Some(addr) = url.strip_prefix("tcp://") {
            Stream::Tcp(TcpStream::connect(addr)?)
        } else {
            #[cfg(unix)]
            {
                Stream::Unix(std::os::unix::net::UnixStream::connect(url)?)
            }
            #[cfg(not(unix))]
            {
                return Err(io::Error::other(format!("この OS では Unix ソケットにつなげません: {url}")));
            }
        };
        Ok(Client { stream, tree: Tree::new(), session_id: None })
    }

    fn send(&mut self, m: Message) -> io::Result<()> {
        write_message(&mut self.stream, &m.to_value())
    }

    fn read_message(&mut self) -> io::Result<Packet> {
        read_packet(&mut self.stream)
    }

    /// Registers with Python and takes the Page tree. Returns Python's error
    /// text as an error.
    pub fn register(&mut self, page: Value) -> io::Result<RegisterResponse> {
        self.send(Message::new(
            Action::RegisterClient,
            RegisterRequest { session_id: self.session_id.clone(), page_name: String::new(), page }.to_value(),
        ))?;
        loop {
            match self.read_message()? {
                Packet::Message(v) => {
                    let m = Message::from_value(v).map_err(io::Error::other)?;
                    if m.action != Action::RegisterClient {
                        continue;
                    }
                    let r = RegisterResponse::from_value(&m.body).map_err(io::Error::other)?;
                    if let Some(e) = &r.error {
                        return Err(io::Error::other(format!("登録を断られました: {e}")));
                    }
                    self.session_id = r.session_id.clone();
                    match self.tree.root {
                        None => {
                            self.tree.set_root(&r.page_patch).map_err(io::Error::other)?;
                        }
                        Some(root) => {
                            self.tree.update(root, &r.page_patch).map_err(io::Error::other)?;
                        }
                    }
                    return Ok(r);
                }
                _ => continue,
            }
        }
    }

    /// Waits for the next thing from Python and applies it to the tree
    pub fn next(&mut self) -> io::Result<Incoming> {
        loop {
            return Ok(match self.read_message()? {
                Packet::Message(v) => {
                    let m = Message::from_value(v).map_err(io::Error::other)?;
                    match m.action {
                        Action::PatchControl => {
                            let p = PatchControl::from_value(&m.body).map_err(io::Error::other)?;
                            let changed = self.tree.apply_patch(p.id, &p.patch).map_err(io::Error::other)?;
                            Incoming::Patched { root: p.id, changed }
                        }
                        Action::InvokeMethod => Incoming::Invoke(InvokeRequest::from_value(&m.body).map_err(io::Error::other)?),
                        Action::SessionCrashed => Incoming::Crashed(SessionCrashed::from_value(&m.body).message),
                        Action::PythonOutput => Incoming::Output {
                            text: m.body.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                            is_stderr: m.body.get("is_stderr").and_then(Value::as_bool).unwrap_or(false),
                        },
                        _ => Incoming::Other(m),
                    }
                }
                Packet::Data { channel, bytes } => Incoming::Data { channel, bytes },
                Packet::Unknown(_) => continue,
            });
        }
    }

    /// Sends an event of a control (`name` without `on_`)
    pub fn send_event(&mut self, target: ControlId, name: &str, data: Value) -> io::Result<()> {
        self.send(Message::new(Action::ControlEvent, ControlEvent { target, name: name.to_string(), data }.to_value()))
    }

    /// Writes properties the client changed back to Python's control (and
    /// into the tree here)
    pub fn update_props(&mut self, id: ControlId, props: Vec<(&str, Value)>) -> io::Result<()> {
        let v = Value::map(props);
        self.tree.update(id, &v).map_err(io::Error::other)?;
        self.send(Message::new(Action::UpdateControlProps, UpdateProps { id, props: v }.to_value()))
    }

    /// Answers an [`Incoming::Invoke`]
    pub fn invoke_result(&mut self, req: &InvokeRequest, result: Value, error: Option<String>) -> io::Result<()> {
        self.send(Message::new(
            Action::InvokeMethod,
            InvokeResponse { control_id: req.control_id, call_id: req.call_id.clone(), result, error }.to_value(),
        ))
    }
}
