//! **A client on a socket**: connects to the page URL a Flet program hands
//! over (`flet <url> <pid file>`), registers, reads what Python sends, and
//! sends events, property updates and method results.
//!
//! Reading blocks, so the app reads on a thread of its own ([`Client::next`])
//! and applies the patches to its [`Tree`] where it draws. Sending goes
//! through a [`Sender`], a clone of the socket's writing half behind a lock,
//! so the drawing side sends events while the reading thread waits.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

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

impl Stream {
    fn try_clone(&self) -> io::Result<Stream> {
        Ok(match self {
            Stream::Tcp(s) => Stream::Tcp(s.try_clone()?),
            #[cfg(unix)]
            Stream::Unix(s) => Stream::Unix(s.try_clone()?),
        })
    }
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
    /// A patch to apply to the tree with [`Tree::apply_patch`]
    Patch(PatchControl),
    /// Python asks for a control's method; answer with [`Sender::invoke_result`]
    Invoke(InvokeRequest),
    Crashed(String),
    /// A line of Python's output (Pyodide only)
    Output { text: String, is_stderr: bool },
    /// A data channel frame
    Data { channel: u32, bytes: Vec<u8> },
    /// A message this client has no use for
    Other(Message),
}

/// The writing half; clone it freely
#[derive(Clone)]
pub struct Sender(Arc<Mutex<Stream>>);

impl Sender {
    fn send(&self, m: Message) -> io::Result<()> {
        let mut s = self.0.lock().map_err(|_| io::Error::other("送る側の錠が壊れています"))?;
        write_message(&mut *s, &m.to_value())
    }

    /// Sends an event of a control (`name` without `on_`)
    pub fn send_event(&self, target: ControlId, name: &str, data: Value) -> io::Result<()> {
        self.send(Message::new(Action::ControlEvent, ControlEvent { target, name: name.to_string(), data }.to_value()))
    }

    /// Writes properties the client changed back to Python's control. The
    /// caller puts the same into its tree ([`Tree::update`]).
    pub fn update_props(&self, id: ControlId, props: Value) -> io::Result<()> {
        self.send(Message::new(Action::UpdateControlProps, UpdateProps { id, props }.to_value()))
    }

    /// Answers an [`Incoming::Invoke`]
    pub fn invoke_result(&self, req: &InvokeRequest, result: Value, error: Option<String>) -> io::Result<()> {
        self.send(Message::new(
            Action::InvokeMethod,
            InvokeResponse { control_id: req.control_id, call_id: req.call_id.clone(), result, error }.to_value(),
        ))
    }
}

pub struct Client {
    reader: Stream,
    sender: Sender,
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
        let reader = if let Some(addr) = url.strip_prefix("tcp://") {
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
        let writer = reader.try_clone()?;
        Ok(Client { reader, sender: Sender(Arc::new(Mutex::new(writer))), session_id: None })
    }

    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }

    /// Registers with Python. The response carries the whole Page; put it
    /// in a tree with [`Tree::set_root`] (or [`Tree::update`] when
    /// reconnecting). Python's error text comes back as an error.
    pub fn register(&mut self, page: Value) -> io::Result<RegisterResponse> {
        self.sender.send(Message::new(
            Action::RegisterClient,
            RegisterRequest { session_id: self.session_id.clone(), page_name: String::new(), page }.to_value(),
        ))?;
        loop {
            if let Packet::Message(v) = read_packet(&mut self.reader)? {
                let m = Message::from_value(v).map_err(io::Error::other)?;
                if m.action != Action::RegisterClient {
                    continue;
                }
                let r = RegisterResponse::from_value(&m.body).map_err(io::Error::other)?;
                if let Some(e) = &r.error {
                    return Err(io::Error::other(format!("登録を断られました: {e}")));
                }
                self.session_id = r.session_id.clone();
                return Ok(r);
            }
        }
    }

    /// Waits for the next thing from Python. An `UnexpectedEof` means
    /// Python closed the socket (the program ended).
    pub fn next(&mut self) -> io::Result<Incoming> {
        loop {
            return Ok(match read_packet(&mut self.reader)? {
                Packet::Message(v) => {
                    let m = Message::from_value(v).map_err(io::Error::other)?;
                    match m.action {
                        Action::PatchControl => Incoming::Patch(PatchControl::from_value(&m.body).map_err(io::Error::other)?),
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
}

/// Applies what arrived to a tree. Returns the controls that changed, or
/// None when it was not a patch.
pub fn apply(tree: &mut Tree, incoming: &Incoming) -> Result<Option<Vec<ControlId>>, String> {
    match incoming {
        Incoming::Patch(p) => tree.apply_patch(p.id, &p.patch).map(Some),
        _ => Ok(None),
    }
}
