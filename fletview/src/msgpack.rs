//! **MessagePack values as Flet exchanges them.**
//!
//! A general value type, read and written by hand on top of `rmp`'s markers.
//! Maps keep their order and take keys of any kind: a patch's tree index has
//! integer keys, control properties string keys. Flet's extension types
//! (1 a date-time, 2 a time of day, 3 a duration; all carried as text) are
//! kept as [`Value::Ext`] and read with [`Value::ext_text`].

use std::fmt;
use std::io::{self, Read, Write};

use rmp::encode;
use rmp::Marker;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    UInt(u64),
    F64(f64),
    Str(String),
    Bin(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    /// An extension type: the type code and its bytes
    Ext(i8, Vec<u8>),
}

/// Extension type 1: a date-time as ISO 8601 text (UTC, `+00:00`)
pub const EXT_DATETIME: i8 = 1;
/// Extension type 2: a time of day as `hour:minute`
pub const EXT_TIME: i8 = 2;
/// Extension type 3: a duration as microseconds, in text
pub const EXT_DURATION: i8 = 3;

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(s.to_string())
    }

    /// A map with string keys, in the given order
    pub fn map(pairs: Vec<(&str, Value)>) -> Value {
        Value::Map(pairs.into_iter().map(|(k, v)| (Value::str(k), v)).collect())
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::UInt(n) => i64::try_from(*n).ok(),
            Value::F64(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::UInt(n) => Some(*n as f64),
            Value::F64(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(Value, Value)]> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// The entry of a map whose key is the string `key`
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map()?.iter().find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v)
    }

    /// The text of an extension value (Flet carries all three as text)
    pub fn ext_text(&self) -> Option<(i8, String)> {
        match self {
            Value::Ext(t, b) => Some((*t, String::from_utf8_lossy(b).into_owned())),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error(format!("読めません: {e}"))
    }
}

fn read_exact<R: Read>(rd: &mut R, n: usize) -> Result<Vec<u8>, Error> {
    let mut buf = vec![0u8; n];
    rd.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_u8<R: Read>(rd: &mut R) -> Result<u8, Error> {
    Ok(read_exact(rd, 1)?[0])
}

fn read_u16<R: Read>(rd: &mut R) -> Result<u16, Error> {
    Ok(u16::from_be_bytes(read_exact(rd, 2)?.try_into().expect("2 bytes")))
}

fn read_u32<R: Read>(rd: &mut R) -> Result<u32, Error> {
    Ok(u32::from_be_bytes(read_exact(rd, 4)?.try_into().expect("4 bytes")))
}

fn read_u64<R: Read>(rd: &mut R) -> Result<u64, Error> {
    Ok(u64::from_be_bytes(read_exact(rd, 8)?.try_into().expect("8 bytes")))
}

fn read_str<R: Read>(rd: &mut R, n: usize) -> Result<Value, Error> {
    let b = read_exact(rd, n)?;
    String::from_utf8(b).map(Value::Str).map_err(|e| Error(format!("UTF-8 でない字: {e}")))
}

fn read_ext<R: Read>(rd: &mut R, n: usize) -> Result<Value, Error> {
    let t = read_u8(rd)? as i8;
    Ok(Value::Ext(t, read_exact(rd, n)?))
}

/// Reads one value
pub fn read_value<R: Read>(rd: &mut R) -> Result<Value, Error> {
    let m = Marker::from_u8(read_u8(rd)?);
    Ok(match m {
        Marker::Null => Value::Nil,
        Marker::True => Value::Bool(true),
        Marker::False => Value::Bool(false),
        Marker::FixPos(n) => Value::Int(n as i64),
        Marker::FixNeg(n) => Value::Int(n as i64),
        Marker::U8 => Value::UInt(read_u8(rd)? as u64),
        Marker::U16 => Value::UInt(read_u16(rd)? as u64),
        Marker::U32 => Value::UInt(read_u32(rd)? as u64),
        Marker::U64 => Value::UInt(read_u64(rd)?),
        Marker::I8 => Value::Int(read_u8(rd)? as i8 as i64),
        Marker::I16 => Value::Int(read_u16(rd)? as i16 as i64),
        Marker::I32 => Value::Int(read_u32(rd)? as i32 as i64),
        Marker::I64 => Value::Int(read_u64(rd)? as i64),
        Marker::F32 => Value::F64(f32::from_bits(read_u32(rd)?) as f64),
        Marker::F64 => Value::F64(f64::from_bits(read_u64(rd)?)),
        Marker::FixStr(n) => read_str(rd, n as usize)?,
        Marker::Str8 => {
            let n = read_u8(rd)? as usize;
            read_str(rd, n)?
        }
        Marker::Str16 => {
            let n = read_u16(rd)? as usize;
            read_str(rd, n)?
        }
        Marker::Str32 => {
            let n = read_u32(rd)? as usize;
            read_str(rd, n)?
        }
        Marker::Bin8 => {
            let n = read_u8(rd)? as usize;
            Value::Bin(read_exact(rd, n)?)
        }
        Marker::Bin16 => {
            let n = read_u16(rd)? as usize;
            Value::Bin(read_exact(rd, n)?)
        }
        Marker::Bin32 => {
            let n = read_u32(rd)? as usize;
            Value::Bin(read_exact(rd, n)?)
        }
        Marker::FixArray(n) => read_array(rd, n as usize)?,
        Marker::Array16 => {
            let n = read_u16(rd)? as usize;
            read_array(rd, n)?
        }
        Marker::Array32 => {
            let n = read_u32(rd)? as usize;
            read_array(rd, n)?
        }
        Marker::FixMap(n) => read_map(rd, n as usize)?,
        Marker::Map16 => {
            let n = read_u16(rd)? as usize;
            read_map(rd, n)?
        }
        Marker::Map32 => {
            let n = read_u32(rd)? as usize;
            read_map(rd, n)?
        }
        Marker::FixExt1 => read_ext(rd, 1)?,
        Marker::FixExt2 => read_ext(rd, 2)?,
        Marker::FixExt4 => read_ext(rd, 4)?,
        Marker::FixExt8 => read_ext(rd, 8)?,
        Marker::FixExt16 => read_ext(rd, 16)?,
        Marker::Ext8 => {
            let n = read_u8(rd)? as usize;
            read_ext(rd, n)?
        }
        Marker::Ext16 => {
            let n = read_u16(rd)? as usize;
            read_ext(rd, n)?
        }
        Marker::Ext32 => {
            let n = read_u32(rd)? as usize;
            read_ext(rd, n)?
        }
        Marker::Reserved => return Err(Error("使われていない印(0xc1)".into())),
    })
}

fn read_array<R: Read>(rd: &mut R, n: usize) -> Result<Value, Error> {
    let mut out = Vec::with_capacity(n.min(1 << 16));
    for _ in 0..n {
        out.push(read_value(rd)?);
    }
    Ok(Value::Array(out))
}

fn read_map<R: Read>(rd: &mut R, n: usize) -> Result<Value, Error> {
    let mut out = Vec::with_capacity(n.min(1 << 16));
    for _ in 0..n {
        let k = read_value(rd)?;
        let v = read_value(rd)?;
        out.push((k, v));
    }
    Ok(Value::Map(out))
}

/// Reads one value from a slice; the whole slice must be that value
pub fn decode(bytes: &[u8]) -> Result<Value, Error> {
    let mut rd = bytes;
    let v = read_value(&mut rd)?;
    if !rd.is_empty() {
        return Err(Error(format!("値の後ろに {} バイト余っています", rd.len())));
    }
    Ok(v)
}

fn werr<E: fmt::Debug>(e: E) -> io::Error {
    io::Error::other(format!("{e:?}"))
}

/// Writes one value
pub fn write_value<W: Write>(wr: &mut W, v: &Value) -> io::Result<()> {
    match v {
        Value::Nil => encode::write_nil(wr).map_err(werr)?,
        Value::Bool(b) => encode::write_bool(wr, *b).map_err(werr)?,
        Value::Int(n) => encode::write_sint(wr, *n).map(|_| ()).map_err(werr)?,
        Value::UInt(n) => encode::write_uint(wr, *n).map(|_| ()).map_err(werr)?,
        Value::F64(f) => encode::write_f64(wr, *f).map_err(werr)?,
        Value::Str(s) => encode::write_str(wr, s).map_err(werr)?,
        Value::Bin(b) => encode::write_bin(wr, b).map_err(werr)?,
        Value::Array(a) => {
            encode::write_array_len(wr, a.len() as u32).map(|_| ()).map_err(werr)?;
            for x in a {
                write_value(wr, x)?;
            }
        }
        Value::Map(m) => {
            encode::write_map_len(wr, m.len() as u32).map(|_| ()).map_err(werr)?;
            for (k, x) in m {
                write_value(wr, k)?;
                write_value(wr, x)?;
            }
        }
        Value::Ext(t, b) => {
            encode::write_ext_meta(wr, b.len() as u32, *t).map(|_| ()).map_err(werr)?;
            wr.write_all(b)?;
        }
    }
    Ok(())
}

/// One value as bytes
pub fn encode(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(&mut out, v).expect("writing to a Vec cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        let v = Value::Array(vec![
            Value::Int(2),
            Value::map(vec![
                ("id", Value::Int(1)),
                ("neg", Value::Int(-40)),
                ("big", Value::Int(-70_000)),
                ("u", Value::UInt(300)),
                ("f", Value::F64(1.5)),
                ("s", Value::str("押す前")),
                ("nil", Value::Nil),
                ("b", Value::Bool(true)),
                ("bin", Value::Bin(vec![0, 1, 2])),
                ("ext", Value::Ext(EXT_TIME, b"9:30".to_vec())),
                ("tree", Value::Map(vec![(Value::Int(0), Value::Array(vec![Value::Int(2)]))])),
            ]),
        ]);
        let bytes = encode(&v);
        assert_eq!(decode(&bytes).unwrap(), v);
    }

    #[test]
    fn the_real_click_event_encodes_as_python_reads_it() {
        // [3, {"target": 10, "name": "click", "data": null}] as msgpack
        let v = Value::Array(vec![
            Value::Int(3),
            Value::map(vec![("target", Value::Int(10)), ("name", Value::str("click")), ("data", Value::Nil)]),
        ]);
        let bytes = encode(&v);
        assert_eq!(bytes[0], 0x92); // fixarray of 2
        assert_eq!(bytes[1], 0x03);
        assert_eq!(bytes[2], 0x83); // fixmap of 3
        assert_eq!(decode(&bytes).unwrap(), v);
    }

    #[test]
    fn an_extension_keeps_its_type_and_text() {
        let v = Value::Ext(EXT_DATETIME, b"2026-10-17T09:30:00+00:00".to_vec());
        let back = decode(&encode(&v)).unwrap();
        assert_eq!(back.ext_text(), Some((1, "2026-10-17T09:30:00+00:00".to_string())));
    }
}
