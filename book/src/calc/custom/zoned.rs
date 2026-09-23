//! `ZONED` and `TO_ZONE`: moments with a time zone (2026-09-24,
//! docs/sekkei/time-zone.ja.adoc).
//!
//! ```text
//! =ZONED("2026-10-01 10:00", "Asia/Tokyo")          departure, Tokyo time
//! =ZONED("2026-10-01 04:30", "America/Los_Angeles") arrival, LA time
//! =B2-B1                                           0.4375 (10 h 30 min)
//! =TO_ZONE(B2, "Asia/Tokyo")                       the arrival, Tokyo time
//! ```
//!
//! The value is [`Value::Zoned`]: arithmetic sees the moment in UTC, and the
//! cell shows the clock time in its zone.

use super::Ctx;
use crate::calc::funcs::excel_epoch;
use crate::Value;

/// ZONED(clock time, zone): the moment the clock in `zone` shows that time.
/// The time is text (`2026-10-01 10:00`) or a serial date-time. An empty
/// zone is the workbook's zone.
pub(super) fn zoned(a: &[Value], ctx: &Ctx) -> Value {
    let ep = excel_epoch(ctx.date1904);
    let Some(zone) = zone_arg(a.get(1)) else { return bad() };
    let local = match a.first() {
        Some(Value::Text(t)) => parse_clock_text(t, ep),
        Some(v @ Value::Zoned { .. }) => Some(v.local_serial()),
        Some(v) => Some(v.as_number()),
        None => None,
    };
    local.and_then(|l| zoned_from_local(l, &zone, ep)).unwrap_or_else(bad)
}

/// TO_ZONE(value, zone): the same moment shown in another zone. A plain
/// serial date-time or text is taken as the workbook zone's clock time.
pub(super) fn to_zone(a: &[Value], ctx: &Ctx) -> Value {
    let ep = excel_epoch(ctx.date1904);
    let Some(zone) = zone_arg(a.get(1)) else { return bad() };
    let here = crate::tz::calc_zone();
    let from = match a.first() {
        Some(v @ Value::Zoned { .. }) => Some(v.clone()),
        Some(Value::Text(t)) => parse_clock_text(t, ep).and_then(|l| zoned_from_local(l, &here, ep)),
        Some(v) => zoned_from_local(v.as_number(), &here, ep),
        None => None,
    };
    match from {
        Some(Value::Zoned { unix, serial, .. }) => Value::Zoned { unix, serial, zone },
        _ => bad(),
    }
}

fn bad() -> Value {
    Value::Error("#VALUE!".into())
}

/// The zone argument: an IANA name, or the workbook's zone when it is left
/// out or empty. None when the name is unknown.
fn zone_arg(v: Option<&Value>) -> Option<String> {
    let z = v.map(|v| v.display()).unwrap_or_default();
    let z = z.trim();
    if z.is_empty() {
        Some(crate::tz::calc_zone())
    } else if crate::tz::is_zone(z) {
        Some(z.to_string())
    } else {
        None
    }
}

/// A clock time written as text, as a serial date-time with the epoch `ep`.
/// `2026-10-01 10:00`, `2026/10/01 10:00:30`, `2026-10-01T10:00` and a date
/// alone are read.
fn parse_clock_text(t: &str, ep: i64) -> Option<f64> {
    let t = t.trim().replace('/', "-").replace('T', " ");
    let when = ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| chrono::NaiveDateTime::parse_from_str(&t, f).ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(&t, "%Y-%m-%d").ok()?.and_hms_opt(0, 0, 0))?;
    let utc = when.and_utc();
    let secs = utc.timestamp() as f64 + utc.timestamp_subsec_millis() as f64 / 1000.0;
    Some(secs / 86400.0 + ep as f64)
}

/// The moment whose clock time in `zone` is the serial date-time `local`
/// (counted with the workbook's epoch `ep`).
pub(crate) fn zoned_from_local(local: f64, zone: &str, ep: i64) -> Option<Value> {
    let secs = ((local - ep as f64) * 86400.0 * 1000.0).round() / 1000.0;
    let naive = chrono::DateTime::from_timestamp(secs.floor() as i64, 0)?.naive_utc()
        + chrono::Duration::milliseconds(((secs - secs.floor()) * 1000.0).round() as i64);
    let unix = crate::tz::moment_of(naive, zone)?;
    Some(Value::Zoned { unix, serial: unix / 86400.0 + ep as f64, zone: zone.to_string() })
}

#[cfg(test)]
mod tests {
    use crate::calc::recalc;
    use crate::{Cell, Pos, Sheet, Value};

    fn sheet(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet::new("旅程");
        for (a1, t) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(t));
        }
        recalc(&mut s);
        s
    }

    #[test]
    fn a_flight_across_zones_takes_the_real_time() {
        let s = sheet(&[
            ("A1", r#"=ZONED("2026-10-01 10:00", "Asia/Tokyo")"#),
            ("A2", r#"=ZONED("2026-10-01 04:30", "America/Los_Angeles")"#),
            ("A3", "=A2-A1"),
            ("A4", "=HOUR(A2)"),
            ("A5", r#"=TO_ZONE(A2, "Asia/Tokyo")"#),
            ("A6", "=HOUR(A5)"),
        ]);
        let v = |a1: &str| s.value(Pos::parse(a1).unwrap());
        assert_eq!(v("A1").display(), "2026-10-01 10:00 Asia/Tokyo");
        assert_eq!(v("A2").display(), "2026-10-01 04:30 America/Los_Angeles");
        // 10:00 JST is 01:00 UTC; 04:30 PDT is 11:30 UTC: 10 h 30 min
        let d = v("A3").as_number();
        assert!((d - 10.5 / 24.0).abs() < 1e-9, "the flight took {} h", d * 24.0);
        assert_eq!(v("A4"), Value::Number(4.0), "HOUR reads LA's clock");
        assert_eq!(v("A5").display(), "2026-10-01 20:30 Asia/Tokyo");
        assert_eq!(v("A6"), Value::Number(20.0));
        // a number format is applied to the clock time in the zone
        assert_eq!(crate::fmt::format_value(&v("A2"), Some("m/d h:mm"), false), "10/1 4:30");
        assert_eq!(crate::fmt::format_value(&v("A2"), Some("General"), false),
                   "2026-10-01 04:30 America/Los_Angeles");
        // the value kept in xlsx is the clock time in its zone
        assert!((v("A2").local_serial() - (46296.0 + 4.5 / 24.0)).abs() < 1e-9);
    }

    #[test]
    fn a_clock_time_that_does_not_happen_is_an_error() {
        // 2026-03-08 02:30 does not happen in Los Angeles (02:00 -> 03:00)
        let s = sheet(&[("A1", r#"=ZONED("2026-03-08 02:30", "America/Los_Angeles")"#),
                        ("A2", r#"=ZONED("2026-10-01 10:00", "Mars/Olympus")"#)]);
        assert_eq!(s.value(Pos::parse("A1").unwrap()), Value::Error("#VALUE!".into()));
        assert_eq!(s.value(Pos::parse("A2").unwrap()), Value::Error("#VALUE!".into()));
    }

    #[test]
    fn a_registered_function_is_called_and_a_built_in_one_wins() {
        fn twice(a: &[Value], _: &super::super::Ctx) -> Value {
            Value::Number(a.first().map(|v| v.as_number()).unwrap_or(0.0) * 2.0)
        }
        super::super::register("twice_test", twice);
        super::super::register("SUM", twice);
        let s = sheet(&[("A1", "21"), ("A2", "=TWICE_TEST(A1)"), ("A3", "=SUM(A1)")]);
        assert_eq!(s.value(Pos::parse("A2").unwrap()), Value::Number(42.0));
        assert_eq!(s.value(Pos::parse("A3").unwrap()), Value::Number(21.0));
    }
}
