//! **The packets on the socket.**
//!
//! Every packet is `[length u32 LE][kind u8][body]`; the length counts the
//! kind and the body. Kind `0x00` is a MessagePack message `[action, body]`,
//! kind `0x01` a data channel frame `[channel u32 LE][bytes]` (raw images
//! and the like; the first controls do not use it, so it is handed over
//! and may be dropped).

use std::io::{self, Read, Write};

use crate::msgpack::{self, Value};

pub const KIND_MESSAGE: u8 = 0x00;
pub const KIND_DATA: u8 = 0x01;

#[derive(Debug)]
pub enum Packet {
    Message(Value),
    Data { channel: u32, bytes: Vec<u8> },
    /// A kind this crate does not know; the Dart client drops these too
    Unknown(u8),
}

/// Reads one packet. An EOF before any byte is `UnexpectedEof`.
pub fn read_packet<R: Read>(rd: &mut R) -> io::Result<Packet> {
    let mut head = [0u8; 4];
    rd.read_exact(&mut head)?;
    let len = u32::from_le_bytes(head) as usize;
    if len == 0 {
        return Ok(Packet::Unknown(0xff));
    }
    let mut body = vec![0u8; len];
    rd.read_exact(&mut body)?;
    Ok(match body[0] {
        KIND_MESSAGE => Packet::Message(msgpack::decode(&body[1..]).map_err(|e| io::Error::other(e.0))?),
        KIND_DATA if len >= 5 => Packet::Data {
            channel: u32::from_le_bytes(body[1..5].try_into().expect("4 bytes")),
            bytes: body[5..].to_vec(),
        },
        other => Packet::Unknown(other),
    })
}

fn write_framed<W: Write>(wr: &mut W, kind: u8, body: &[u8]) -> io::Result<()> {
    let len = (body.len() + 1) as u32;
    wr.write_all(&len.to_le_bytes())?;
    wr.write_all(&[kind])?;
    wr.write_all(body)?;
    wr.flush()
}

/// Writes one message packet
pub fn write_message<W: Write>(wr: &mut W, value: &Value) -> io::Result<()> {
    write_framed(wr, KIND_MESSAGE, &msgpack::encode(value))
}

/// Writes one data channel frame
pub fn write_data<W: Write>(wr: &mut W, channel: u32, bytes: &[u8]) -> io::Result<()> {
    let mut body = channel.to_le_bytes().to_vec();
    body.extend_from_slice(bytes);
    write_framed(wr, KIND_DATA, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_round_trips_through_the_frame() {
        let v = Value::Array(vec![Value::Int(3), Value::map(vec![("target", Value::Int(10))])]);
        let mut buf = Vec::new();
        write_message(&mut buf, &v).unwrap();
        assert_eq!(&buf[..4], &((buf.len() - 4) as u32).to_le_bytes());
        assert_eq!(buf[4], KIND_MESSAGE);
        let mut rd = &buf[..];
        match read_packet(&mut rd).unwrap() {
            Packet::Message(m) => assert_eq!(m, v),
            other => panic!("{other:?}"),
        }
        assert!(rd.is_empty());
    }

    #[test]
    fn a_data_frame_carries_its_channel() {
        let mut buf = Vec::new();
        write_data(&mut buf, 7, b"xyz").unwrap();
        let mut rd = &buf[..];
        match read_packet(&mut rd).unwrap() {
            Packet::Data { channel, bytes } => {
                assert_eq!(channel, 7);
                assert_eq!(bytes, b"xyz");
            }
            other => panic!("{other:?}"),
        }
    }
}
