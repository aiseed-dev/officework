//! **The client side of Flet's protocol** (SEKKEI "検討: officework を Flet の
//! 画面側にする", 2026-10-17; the rules are copied in
//! docs/sekkei/flet-client.ja.adoc).
//!
//! A Flet program (`ft.run(main)`) starts a client and talks to it over a
//! socket: it registers, receives the control tree as patches, sends events
//! and property updates, and answers method calls. This crate is that
//! client without the drawing: [`msgpack`] reads and writes the values,
//! [`wire`] the packets, [`message`] the seven kinds of message, [`tree`]
//! keeps the controls and applies the patches, and [`client`] ties them to
//! a socket. What a control looks like on screen is decided elsewhere, by
//! the gpui side, which reads the tree.
//!
//! Nothing here depends on gpui, the same wall as `face`.

pub mod client;
pub mod message;
pub mod msgpack;
pub mod tree;
pub mod wire;

pub use client::{Client, Incoming};
pub use message::{Action, Message};
pub use msgpack::Value;
pub use tree::{Control, ControlId, Prop, Tree};
