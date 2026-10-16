//! **Arrays to and from Python through the buffer protocol** (SEKKEI
//! "決め: 部品へのデータとコードの入り口は 3 つ", 2026-10-17, the second way).
//!
//! A numpy array of numbers is read straight from its buffer (through
//! `memoryview`), so numpy is never imported and its version does not
//! matter. Anything else (strings,
//! objects, datetime64, plain lists) goes through `tolist()` and the one
//! conversion `ws["A1"] = v` uses. A one-dimensional array is one column,
//! the way a spilled one-dimensional result is.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};

use book::{Pos, Value};

use crate::arrow::put;
use crate::{cell_of, to_out, Out};

/// Writes `arr` at `origin`. Returns (rows, columns) written.
pub fn write(sheet: &mut book::Sheet, arr: &Bound<'_, PyAny>, origin: Pos, d1904: bool) -> PyResult<(u32, u32)> {
    if let Some((vals, shape)) = numeric(arr)? {
        place(sheet, &vals, shape, origin, None);
        return Ok(shape);
    }
    lists(sheet, arr, origin, d1904)
}

/// The values of an array of numbers or booleans, read from its buffer
/// through `memoryview` (the buffer API itself is outside the limited
/// Python API before 3.11), with the shape (rows, columns). None when the
/// object has no buffer or holds something else (strings, objects).
fn numeric(arr: &Bound<'_, PyAny>) -> PyResult<Option<(Vec<Value>, (u32, u32))>> {
    let py = arr.py();
    let Ok(mv) = py.import("builtins")?.getattr("memoryview")?.call1((arr,)) else { return Ok(None) };
    let fmt: String = mv.getattr("format")?.extract()?;
    let itemsize: usize = mv.getattr("itemsize")?.extract()?;
    let shape: Vec<usize> = mv.getattr("shape")?.extract()?;
    // struct syntax: an optional byte order, then one code
    let code = fmt.trim_start_matches(['<', '>', '=', '@', '!']).chars().next().unwrap_or(' ');
    let shape = shape_of(&shape)?;
    let bytes: Vec<u8> = mv.call_method0("tobytes")?.extract()?;
    let n = if itemsize == 0 { 0 } else { bytes.len() / itemsize };
    macro_rules! read_as {
        ($t:ty, $f:expr) => {{
            bytes
                .chunks_exact(std::mem::size_of::<$t>())
                .take(n)
                .map(|c| $f(<$t>::from_ne_bytes(c.try_into().expect("chunk size"))))
                .collect::<Vec<Value>>()
        }};
    }
    let num = |x: f64| if x.is_nan() { Value::Empty } else { Value::Number(x) };
    let vals = match (code, itemsize) {
        ('d', 8) => read_as!(f64, num),
        ('f', 4) => read_as!(f32, |x| num(x as f64)),
        ('q', 8) | ('l', 8) | ('n', 8) => read_as!(i64, |x| Value::Number(x as f64)),
        ('i', 4) | ('l', 4) => read_as!(i32, |x| Value::Number(x as f64)),
        ('h', 2) => read_as!(i16, |x| Value::Number(x as f64)),
        ('b', 1) => read_as!(i8, |x| Value::Number(x as f64)),
        ('Q', 8) | ('L', 8) | ('N', 8) => read_as!(u64, |x| Value::Number(x as f64)),
        ('I', 4) | ('L', 4) => read_as!(u32, |x| Value::Number(x as f64)),
        ('H', 2) => read_as!(u16, |x| Value::Number(x as f64)),
        ('B', 1) => read_as!(u8, |x| Value::Number(x as f64)),
        ('?', 1) => read_as!(u8, |x| Value::Bool(x != 0)),
        // 'U' strings, 'O' objects, 'M' / 'm' dates: through tolist()
        _ => return Ok(None),
    };
    Ok(Some((vals, shape)))
}

fn shape_of(shape: &[usize]) -> PyResult<(u32, u32)> {
    match shape {
        [n] => Ok((*n as u32, 1)),
        [r, c] => Ok((*r as u32, *c as u32)),
        other => Err(PyValueError::new_err(format!(
            "置けるのは 1 次元か 2 次元の配列です({} 次元が渡されました)",
            other.len()
        ))),
    }
}

/// Puts row-major values of the given shape at `origin`
fn place(sheet: &mut book::Sheet, vals: &[Value], (rows, cols): (u32, u32), origin: Pos, fmt: Option<&str>) {
    for (i, v) in vals.iter().enumerate() {
        let (r, c) = ((i as u32) / cols.max(1), (i as u32) % cols.max(1));
        if r >= rows {
            break;
        }
        put(sheet, Pos::new(origin.row + r, origin.col + c), v.clone(), fmt);
    }
}

/// Anything with `tolist()`, or a list: rows of Python values, each placed
/// as `ws["A1"] = v` would place it
fn lists(sheet: &mut book::Sheet, arr: &Bound<'_, PyAny>, origin: Pos, d1904: bool) -> PyResult<(u32, u32)> {
    let rows = if arr.hasattr("tolist")? { arr.call_method0("tolist")? } else { arr.clone() };
    let rows = rows
        .cast::<PyList>()
        .map_err(|_| PyTypeError::new_err("配列は numpy の ndarray か、list の list で渡してください"))?;
    let mut n_cols = 0u32;
    let mut n_rows = 0u32;
    for (r, row) in rows.iter().enumerate() {
        // A one-dimensional array: one value per row, one column
        let cells: Vec<Bound<'_, PyAny>> = match row.cast::<PyList>() {
            Ok(l) => l.iter().collect(),
            Err(_) => match row.cast::<PyTuple>() {
                Ok(t) => t.iter().collect(),
                Err(_) => vec![row.clone()],
            },
        };
        n_cols = n_cols.max(cells.len() as u32);
        n_rows = r as u32 + 1;
        for (c, v) in cells.iter().enumerate() {
            let v = if v.is_none() { None } else { Some(v) };
            let (cell, date_fmt) = cell_of(v, d1904)?;
            put(sheet, Pos::new(origin.row + r as u32, origin.col + c as u32), cell.value, date_fmt);
        }
    }
    Ok((n_rows, n_cols))
}

/// Reads `a..=b`. Numbers only: `(bytes, "d" | "q", (rows, cols))` for
/// float64 or int64; booleans only: the same with `"?"`; otherwise the
/// values as a list of lists. The Python side turns these into an ndarray.
pub fn read(py: Python<'_>, sheet: &book::Sheet, a: Pos, b: Pos) -> PyResult<Py<PyAny>> {
    let rows = (b.row - a.row + 1) as usize;
    let cols = (b.col - a.col + 1) as usize;
    let mut vals: Vec<Value> = Vec::with_capacity(rows * cols);
    for r in a.row..=b.row {
        for c in a.col..=b.col {
            vals.push(sheet.value(Pos::new(r, c)));
        }
    }
    let shape = (rows, cols);
    let (mut num, mut empty, mut boolean, mut other, mut integral) = (false, false, false, false, true);
    for v in &vals {
        match v {
            Value::Empty => empty = true,
            Value::Number(x) => {
                num = true;
                integral &= x.fract() == 0.0 && x.abs() < 9.007_199_254_740_992e15;
            }
            Value::Bool(_) => boolean = true,
            _ => other = true,
        }
    }
    if !other && !boolean && (num || empty) {
        if integral && !empty && num {
            let bytes: Vec<u8> = vals.iter().flat_map(|v| (v.as_number() as i64).to_ne_bytes()).collect();
            return Ok((PyBytes::new(py, &bytes), "q", shape).into_pyobject(py)?.into_any().unbind());
        }
        let bytes: Vec<u8> = vals
            .iter()
            .flat_map(|v| match v {
                Value::Number(x) => x.to_ne_bytes(),
                _ => f64::NAN.to_ne_bytes(),
            })
            .collect();
        return Ok((PyBytes::new(py, &bytes), "d", shape).into_pyobject(py)?.into_any().unbind());
    }
    if !other && !num && !empty && boolean {
        let bytes: Vec<u8> = vals.iter().map(|v| matches!(v, Value::Bool(true)) as u8).collect();
        return Ok((PyBytes::new(py, &bytes), "?", shape).into_pyobject(py)?.into_any().unbind());
    }
    let out: Vec<Vec<Option<Out>>> = vals.chunks(cols.max(1)).map(|row| row.iter().map(to_out).collect()).collect();
    Ok(out.into_pyobject(py)?.into_any().unbind())
}
