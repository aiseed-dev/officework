//! **Tables to and from Python through the Arrow C stream interface**
//! (`__arrow_c_stream__`; SEKKEI "決め: Python との表のやり取りは Arrow",
//! 2026-10-17).
//!
//! polars, pyarrow and pandas all export this interface, so nothing here
//! imports any of them, and the Python polars version is free of the Rust
//! one. The Rust side reads and writes with polars-arrow's FFI, which the
//! engine already carries for the pivot tables. Writing 40,000 cells one
//! call at a time took 12 s (the PyO3 call overhead); this hands the whole
//! table over in one call.

use std::ffi::CStr;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyCapsule;

use polars_arrow::array::{
    Array, BooleanArray, DictionaryArray, DictionaryKey, NullArray, PrimitiveArray, StructArray,
    Utf8Array, Utf8ViewArray,
};
use polars_arrow::datatypes::{ArrowDataType, Field, IntegerType, TimeUnit};
use polars_arrow::ffi::{self, ArrowArrayStream, ArrowArrayStreamReader};
use polars_arrow::types::NativeType;

use book::calc::excel_epoch;
use book::{Cell, Pos, Value};

/// The capsule name the Arrow PyCapsule interface fixes for a stream
const CAPSULE: &CStr = c"arrow_array_stream";

/// Number formats put on a cell that states none, the same ones
/// `ws["A1"] = date` puts (`PySheet::__setitem__`)
const FMT_DATE: &str = "yyyy/m/d";
const FMT_DATETIME: &str = "yyyy/m/d h:mm";
const FMT_TIME: &str = "h:mm";
const FMT_DURATION: &str = "[h]:mm:ss";

/// A table as it comes through the stream: the fields and, per chunk, one
/// array per column
pub struct Frame {
    pub fields: Vec<Field>,
    pub chunks: Vec<Vec<Box<dyn Array>>>,
}

fn arrow_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("Arrow のデータが読めません: {e}"))
}

/// Takes the table out of an object that exports `__arrow_c_stream__`
pub fn import(obj: &Bound<'_, PyAny>) -> PyResult<Frame> {
    if !obj.hasattr("__arrow_c_stream__")? {
        return Err(PyTypeError::new_err(format!(
            "表は Arrow で出せる物(polars・pyarrow・pandas の DataFrame)を渡してください。渡されたのは {}",
            obj.get_type().name().map(|n| n.to_string()).unwrap_or_default()
        )));
    }
    let cap = obj.call_method0("__arrow_c_stream__")?;
    let cap = cap
        .cast::<PyCapsule>()
        .map_err(|_| PyTypeError::new_err("__arrow_c_stream__ が PyCapsule を返しませんでした"))?;
    let ptr = cap.pointer_checked(Some(CAPSULE))?.cast::<ArrowArrayStream>();
    // Take the stream out of the capsule. What stays behind has no release
    // callback, so the capsule's own destructor leaves it alone and the
    // stream is released once, when the reader drops
    let stream = unsafe { std::ptr::replace(ptr.as_ptr(), ArrowArrayStream::empty()) };
    let mut reader = unsafe { ArrowArrayStreamReader::try_new(Box::new(stream)) }.map_err(arrow_err)?;
    let fields = match reader.field().dtype() {
        ArrowDataType::Struct(f) => f.clone(),
        other => return Err(arrow_err(format!("表ではありません({other:?})"))),
    };
    let mut chunks = Vec::new();
    while let Some(chunk) = unsafe { reader.next() } {
        let chunk = chunk.map_err(arrow_err)?;
        let s = chunk
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| arrow_err("列の束が struct ではありません"))?;
        chunks.push(s.values().to_vec());
    }
    Ok(Frame { fields, chunks })
}

/// Writes the table at `origin`, the column names first when `header`.
/// Formats stay as they are; a date format is put only on a cell that has
/// none. Returns (rows, columns) written, the header counted.
pub fn write(
    sheet: &mut book::Sheet,
    f: &Frame,
    origin: Pos,
    header: bool,
    date1904: bool,
) -> PyResult<(u32, u32)> {
    let ep = excel_epoch(date1904);
    let mut row = origin.row;
    if header {
        for (c, fl) in f.fields.iter().enumerate() {
            put(sheet, Pos::new(row, origin.col + c as u32), Value::Text(fl.name.to_string()), None);
        }
        row += 1;
    }
    for chunk in &f.chunks {
        let n = chunk.first().map(|a| a.len()).unwrap_or(0);
        for (c, arr) in chunk.iter().enumerate() {
            let name = f.fields.get(c).map(|fl| fl.name.as_str()).unwrap_or("");
            let (vals, fmt) = column_values(arr.as_ref(), name, ep)?;
            let col = origin.col + c as u32;
            for (i, v) in vals.into_iter().enumerate() {
                put(sheet, Pos::new(row + i as u32, col), v, fmt);
            }
        }
        row += n as u32;
    }
    Ok((row - origin.row, f.fields.len() as u32))
}

/// Puts a value, keeping the cell's format; a date format only where the
/// cell states none
pub(crate) fn put(sheet: &mut book::Sheet, p: Pos, v: Value, date_fmt: Option<&str>) {
    let mut fmt = sheet.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
    if fmt.number_format.is_none() && !matches!(v, Value::Empty) {
        if let Some(df) = date_fmt {
            fmt.number_format = Some(df.into());
        }
    }
    sheet.set(p, Cell { formula: None, value: v, fmt });
}

fn prim<T: NativeType>(a: &dyn Array, f: impl Fn(T) -> f64) -> Vec<Value> {
    let p = a.as_any().downcast_ref::<PrimitiveArray<T>>().expect("the dtype names this array");
    p.iter().map(|x| x.map_or(Value::Empty, |x| Value::Number(f(*x)))).collect()
}

fn text(s: Option<&str>) -> Value {
    match s {
        // An empty string is an empty cell, as `ws["A1"] = ""` is
        Some(s) if !s.trim().is_empty() => Value::Text(s.to_string()),
        _ => Value::Empty,
    }
}

/// Seconds per unit of a time type
fn per_sec(u: &TimeUnit) -> f64 {
    match u {
        TimeUnit::Second => 1.0,
        TimeUnit::Millisecond => 1e3,
        TimeUnit::Microsecond => 1e6,
        TimeUnit::Nanosecond => 1e9,
    }
}

/// The cells of one column, and the number format that suits them
fn column_values(a: &dyn Array, name: &str, ep: i64) -> PyResult<(Vec<Value>, Option<&'static str>)> {
    use ArrowDataType as T;
    let n = a.len();
    let epf = ep as f64;
    Ok(match a.dtype() {
        T::Null => (vec![Value::Empty; n], None),
        T::Boolean => {
            let b = a.as_any().downcast_ref::<BooleanArray>().expect("Boolean");
            (b.iter().map(|x| x.map_or(Value::Empty, Value::Bool)).collect(), None)
        }
        T::Int8 => (prim::<i8>(a, |x| x as f64), None),
        T::Int16 => (prim::<i16>(a, |x| x as f64), None),
        T::Int32 => (prim::<i32>(a, |x| x as f64), None),
        T::Int64 => (prim::<i64>(a, |x| x as f64), None),
        T::Int128 => (prim::<i128>(a, |x| x as f64), None),
        T::UInt8 => (prim::<u8>(a, |x| x as f64), None),
        T::UInt16 => (prim::<u16>(a, |x| x as f64), None),
        T::UInt32 => (prim::<u32>(a, |x| x as f64), None),
        T::UInt64 => (prim::<u64>(a, |x| x as f64), None),
        T::Float32 => (prim::<f32>(a, |x| x as f64), None),
        T::Float64 => (prim::<f64>(a, |x| x), None),
        T::Decimal(_, scale) => {
            let d = 10f64.powi(*scale as i32);
            (prim::<i128>(a, move |x| x as f64 / d), None)
        }
        T::Decimal32(_, scale) => {
            let d = 10f64.powi(*scale as i32);
            (prim::<i32>(a, move |x| x as f64 / d), None)
        }
        T::Decimal64(_, scale) => {
            let d = 10f64.powi(*scale as i32);
            (prim::<i64>(a, move |x| x as f64 / d), None)
        }
        // Days since 1970 → the workbook's serial date
        T::Date32 => (prim::<i32>(a, move |x| x as f64 + epf), Some(FMT_DATE)),
        T::Date64 => (prim::<i64>(a, move |x| x as f64 / 86_400_000.0 + epf), Some(FMT_DATETIME)),
        T::Timestamp(unit, zone) => {
            let ps = per_sec(unit);
            let zone = zone.as_ref().map(|z| z.to_string());
            let f = move |x: i64| {
                let mut secs = x as f64 / ps;
                // A zoned moment is written as the clock time of its zone
                if let Some(z) = &zone {
                    secs += book::tz::offset_secs(z, secs.floor() as i64) as f64;
                }
                secs / 86_400.0 + epf
            };
            (prim::<i64>(a, f), Some(FMT_DATETIME))
        }
        T::Time32(unit) => {
            let ps = per_sec(unit);
            (prim::<i32>(a, move |x| x as f64 / ps / 86_400.0), Some(FMT_TIME))
        }
        T::Time64(unit) => {
            let ps = per_sec(unit);
            (prim::<i64>(a, move |x| x as f64 / ps / 86_400.0), Some(FMT_TIME))
        }
        T::Duration(unit) => {
            let ps = per_sec(unit);
            (prim::<i64>(a, move |x| x as f64 / ps / 86_400.0), Some(FMT_DURATION))
        }
        T::Utf8 => {
            let s = a.as_any().downcast_ref::<Utf8Array<i32>>().expect("Utf8");
            (s.iter().map(text).collect(), None)
        }
        T::LargeUtf8 => {
            let s = a.as_any().downcast_ref::<Utf8Array<i64>>().expect("LargeUtf8");
            (s.iter().map(text).collect(), None)
        }
        T::Utf8View => {
            let s = a.as_any().downcast_ref::<Utf8ViewArray>().expect("Utf8View");
            (s.iter().map(text).collect(), None)
        }
        // Categorical and Enum come as a dictionary: decode it
        T::Dictionary(key, _, _) => match key {
            IntegerType::Int8 => dict::<i8>(a, name, ep)?,
            IntegerType::Int16 => dict::<i16>(a, name, ep)?,
            IntegerType::Int32 => dict::<i32>(a, name, ep)?,
            IntegerType::Int64 => dict::<i64>(a, name, ep)?,
            IntegerType::UInt8 => dict::<u8>(a, name, ep)?,
            IntegerType::UInt16 => dict::<u16>(a, name, ep)?,
            IntegerType::UInt32 => dict::<u32>(a, name, ep)?,
            IntegerType::UInt64 => dict::<u64>(a, name, ep)?,
            other => return Err(unsupported(name, &format!("Dictionary({other:?})"))),
        },
        other => return Err(unsupported(name, &format!("{other:?}"))),
    })
}

fn unsupported(name: &str, dtype: &str) -> PyErr {
    PyTypeError::new_err(format!("列「{name}」の型 {dtype} はセルに置けません"))
}

fn dict<K: DictionaryKey>(a: &dyn Array, name: &str, ep: i64) -> PyResult<(Vec<Value>, Option<&'static str>)> {
    let d = a.as_any().downcast_ref::<DictionaryArray<K>>().expect("Dictionary");
    let (vals, fmt) = column_values(d.values().as_ref(), name, ep)?;
    let out = d
        .keys()
        .iter()
        .map(|k| match k {
            Some(k) => (*k).try_into().ok().and_then(|i: usize| vals.get(i).cloned()).unwrap_or(Value::Empty),
            None => Value::Empty,
        })
        .collect();
    Ok((out, fmt))
}

// ---- reading ---------------------------------------------------------------

/// Does this number format show a date, a time, or both? Quoted text and
/// the bracket parts (`[Red]`, `[$-411]`) say nothing; `[h]` says time.
fn date_tokens(code: &str) -> (bool, bool) {
    let mut date = false;
    let mut time = false;
    let mut quoted = false;
    let mut bracket = false;
    let mut prev = '\0';
    for c in code.chars() {
        if quoted {
            quoted = c != '"';
        } else if c == '"' {
            quoted = true;
        } else if bracket {
            if c == ']' {
                bracket = false;
            } else if matches!(c, 'h' | 'H' | 's' | 'S') {
                time = true;
            }
        } else if c == '[' {
            bracket = true;
        } else if prev == '\\' {
            // an escaped character
        } else {
            match c {
                'y' | 'Y' | 'd' | 'D' | 'e' | 'g' | 'E' | 'G' => date = true,
                'h' | 'H' | 's' | 'S' => time = true,
                _ => {}
            }
        }
        prev = c;
    }
    (date, time)
}

/// What one column holds, read off its cells
#[derive(Default)]
struct Seen {
    num: bool,
    integral: bool,
    date: bool,
    time: bool,
    boolean: bool,
    text: bool,
    zone: Option<Option<String>>,
}

impl Seen {
    fn note(&mut self, v: &Value, code: Option<&str>) {
        match v {
            Value::Empty => {}
            Value::Number(n) => {
                if !self.num {
                    self.integral = true;
                }
                self.num = true;
                self.integral &= n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15;
                if let Some(c) = code {
                    let (d, t) = date_tokens(c);
                    self.date |= d;
                    self.time |= t;
                }
            }
            Value::Bool(_) => self.boolean = true,
            Value::Text(_) | Value::Error(_) => self.text = true,
            Value::Zoned { zone, .. } => {
                self.zone = Some(match &self.zone {
                    None => Some(zone.clone()),
                    Some(Some(z)) if z == zone => Some(zone.clone()),
                    _ => None,
                })
            }
        }
    }
}

/// The table as one Arrow struct array; `__arrow_c_stream__` exports it
#[pyclass(name = "Frame", module = "officework._sheet")]
pub struct PyFrame {
    fields: Vec<Field>,
    arrays: Vec<Box<dyn Array>>,
    len: usize,
}

#[pymethods]
impl PyFrame {
    /// The Arrow PyCapsule interface: polars, pyarrow and pandas read it
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        let dtype = ArrowDataType::Struct(self.fields.clone());
        let arrays: Vec<Box<dyn Array>> = self.arrays.iter().map(|a| a.to_boxed()).collect();
        let s = StructArray::try_new(dtype.clone(), self.len, arrays, None).map_err(arrow_err)?;
        let field = Field::new("".into(), dtype, false);
        let stream = ffi::export_iterator(Box::new(std::iter::once(Ok(s.boxed()))), field);
        PyCapsule::new_with_value(py, stream, CAPSULE)
    }

    /// (rows, columns), as a DataFrame's shape
    #[getter]
    fn shape(&self) -> (usize, usize) {
        (self.len, self.fields.len())
    }

    /// The column names
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.name.to_string()).collect()
    }
}

/// Reads the cells `a..=b` as a table. With `header`, the first row names
/// the columns (an empty cell names its column `column_N`).
pub fn read(sheet: &book::Sheet, a: Pos, b: Pos, header: bool, date1904: bool) -> PyResult<PyFrame> {
    let ep = excel_epoch(date1904);
    let first = if header { a.row + 1 } else { a.row };
    let len = (b.row + 1).saturating_sub(first) as usize;
    let mut fields = Vec::new();
    let mut arrays: Vec<Box<dyn Array>> = Vec::new();
    for (i, col) in (a.col..=b.col).enumerate() {
        let name = if header {
            match sheet.value(Pos::new(a.row, col)) {
                Value::Empty => format!("column_{i}"),
                v => v.display(),
            }
        } else {
            format!("column_{i}")
        };
        let cells: Vec<(Value, Option<String>)> = (first..=b.row)
            .map(|r| {
                let p = Pos::new(r, col);
                (sheet.value(p), sheet.get(p).and_then(|c| c.fmt.number_format.clone()))
            })
            .collect();
        let mut seen = Seen::default();
        for (v, code) in &cells {
            seen.note(v, code.as_deref());
        }
        let (dtype, arr): (ArrowDataType, Box<dyn Array>) = column_array(&cells, &seen, ep, date1904);
        fields.push(Field::new(name.into(), dtype, true));
        arrays.push(arr);
    }
    Ok(PyFrame { fields, arrays, len })
}

fn column_array(
    cells: &[(Value, Option<String>)],
    seen: &Seen,
    ep: i64,
    date1904: bool,
) -> (ArrowDataType, Box<dyn Array>) {
    let n = cells.len();
    let kinds = seen.num as u8 + seen.boolean as u8 + seen.text as u8 + seen.zone.is_some() as u8;
    if kinds == 0 {
        return (ArrowDataType::Null, NullArray::new(ArrowDataType::Null, n).boxed());
    }
    if kinds == 1 && seen.num {
        let nums = cells.iter().map(|(v, _)| match v {
            Value::Number(x) => Some(*x),
            _ => None,
        });
        if seen.date && seen.integral && !seen.time {
            let a: PrimitiveArray<i32> = nums.map(|x| x.map(|x| (x as i64 - ep) as i32)).collect();
            return (ArrowDataType::Date32, a.to(ArrowDataType::Date32).boxed());
        }
        if seen.date || seen.time {
            let dt = ArrowDataType::Timestamp(TimeUnit::Microsecond, None);
            let a: PrimitiveArray<i64> =
                nums.map(|x| x.map(|x| ((x - ep as f64) * 86_400.0 * 1e6).round() as i64)).collect();
            return (dt.clone(), a.to(dt).boxed());
        }
        if seen.integral {
            let a: PrimitiveArray<i64> = nums.map(|x| x.map(|x| x as i64)).collect();
            return (ArrowDataType::Int64, a.boxed());
        }
        let a: PrimitiveArray<f64> = nums.collect();
        return (ArrowDataType::Float64, a.boxed());
    }
    if kinds == 1 && seen.boolean {
        let a: BooleanArray = cells
            .iter()
            .map(|(v, _)| match v {
                Value::Bool(b) => Some(*b),
                _ => None,
            })
            .collect();
        return (ArrowDataType::Boolean, a.boxed());
    }
    if let (1, Some(Some(zone))) = (kinds, &seen.zone) {
        let dt = ArrowDataType::Timestamp(TimeUnit::Microsecond, Some(zone.as_str().into()));
        let a: PrimitiveArray<i64> = cells
            .iter()
            .map(|(v, _)| match v {
                Value::Zoned { unix, .. } => Some((unix * 1e6).round() as i64),
                _ => None,
            })
            .collect();
        return (dt.clone(), a.to(dt).boxed());
    }
    // Text, or a mix: the cells as they show
    let a: Utf8Array<i64> = cells
        .iter()
        .map(|(v, code)| match v {
            Value::Empty => None,
            Value::Number(_) => Some(book::format_value(v, code.as_deref(), date1904)),
            v => Some(v.display()),
        })
        .collect();
    (ArrowDataType::LargeUtf8, a.boxed())
}
