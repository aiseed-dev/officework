//! Time zones for dates and times (decided 2026-09-24).
//!
//! A workbook has a time zone ([`crate::Book::time_zone`]). It is an IANA
//! name such as `Asia/Tokyo`; empty means the zone this computer is set to.
//! The zone turns the current moment into the clock time that `NOW()` and
//! `TODAY()` return.
//!
//! The zone database is `chrono-tz`, the same one Polars uses, so that the
//! engine and Polars give the same clock time for the same moment.

use chrono::{Offset, TimeZone};
use std::cell::RefCell;

/// The IANA name of the zone this computer is set to. `UTC` when the
/// system does not say.
pub fn machine_zone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Whether `name` is a zone in the IANA database (`Asia/Tokyo`, `UTC`).
pub fn is_zone(name: &str) -> bool {
    name.parse::<chrono_tz::Tz>().is_ok()
}

/// The zone a workbook setting stands for: the setting itself when it names
/// a known zone, otherwise the computer's zone.
pub fn resolve(setting: &str) -> String {
    let s = setting.trim();
    if !s.is_empty() && is_zone(s) { s.to_string() } else { machine_zone() }
}

/// Seconds to add to UTC to get the clock time in `zone` at the moment
/// `unix_secs`. Daylight saving time is included. An unknown zone counts
/// as UTC.
pub fn offset_secs(zone: &str, unix_secs: i64) -> i64 {
    let Ok(tz) = resolve(zone).parse::<chrono_tz::Tz>() else { return 0 };
    match chrono::DateTime::from_timestamp(unix_secs, 0) {
        Some(utc) => tz.offset_from_utc_datetime(&utc.naive_utc()).fix().local_minus_utc() as i64,
        None => 0,
    }
}

/// A moment shown as the clock time in `zone` and the zone's name:
/// `2026-10-01 10:00 Asia/Tokyo` (seconds are added when there are any).
pub fn show(unix: f64, zone: &str) -> String {
    let secs = unix.round() as i64;
    let Some(utc) = chrono::DateTime::from_timestamp(secs, 0) else { return String::new() };
    let local = utc.naive_utc() + chrono::Duration::seconds(offset_secs(zone, secs));
    let fmt = if secs % 60 == 0 { "%Y-%m-%d %H:%M" } else { "%Y-%m-%d %H:%M:%S" };
    format!("{} {}", local.format(fmt), resolve(zone))
}

/// **The moment of a clock time in `zone`**, in UTC seconds.
///
/// Around daylight saving time a clock time can happen twice (the clock
/// goes back) or not at all (the clock goes forward). Twice: the earlier
/// one is taken. Not at all: None. These are Polars'
/// `replace_time_zone(ambiguous="earliest", non_existent="null")`.
pub fn moment_of(local: chrono::NaiveDateTime, zone: &str) -> Option<f64> {
    let tz = resolve(zone).parse::<chrono_tz::Tz>().ok()?;
    match tz.from_local_datetime(&local) {
        chrono::LocalResult::Single(t) => Some(t.timestamp() as f64),
        chrono::LocalResult::Ambiguous(early, _) => Some(early.timestamp() as f64),
        chrono::LocalResult::None => None,
    }
    .map(|s| s + local.and_utc().timestamp_subsec_millis() as f64 / 1000.0)
}

thread_local! {
    // The zone of the workbook being calculated, set around a recalculation
    // so that NOW() and TODAY() can see it without an extra argument on
    // every function
    static CALC_ZONE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Keeps `zone` as the calculation's zone until it is dropped, then puts
/// back the one before.
pub struct CalcZone(Option<String>);

impl CalcZone {
    pub fn set(zone: &str) -> CalcZone {
        CalcZone(CALC_ZONE.with(|z| z.replace(Some(zone.to_string()))))
    }
}

impl Drop for CalcZone {
    fn drop(&mut self) {
        let before = self.0.take();
        CALC_ZONE.with(|z| *z.borrow_mut() = before);
    }
}

/// The zone the current calculation uses; the computer's zone outside one.
pub fn calc_zone() -> String {
    CALC_ZONE.with(|z| z.borrow().clone()).map(|s| resolve(&s)).unwrap_or_else(machine_zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokyo_is_nine_hours_ahead_all_year() {
        // 2026-01-15 and 2026-07-15, 00:00 UTC
        assert_eq!(offset_secs("Asia/Tokyo", 1_768_435_200), 9 * 3600);
        assert_eq!(offset_secs("Asia/Tokyo", 1_784_073_600), 9 * 3600);
    }

    #[test]
    fn los_angeles_moves_with_daylight_saving_time() {
        assert_eq!(offset_secs("America/Los_Angeles", 1_768_435_200), -8 * 3600);
        assert_eq!(offset_secs("America/Los_Angeles", 1_784_073_600), -7 * 3600);
    }

    #[test]
    fn an_unknown_name_falls_back_to_the_computer() {
        assert!(!is_zone("Mars/Olympus"));
        assert_eq!(resolve("Mars/Olympus"), machine_zone());
        assert_eq!(resolve(""), machine_zone());
        assert_eq!(resolve("Europe/Paris"), "Europe/Paris");
    }

    #[test]
    fn the_calculation_zone_is_put_back() {
        {
            let _g = CalcZone::set("Europe/London");
            assert_eq!(calc_zone(), "Europe/London");
            {
                let _h = CalcZone::set("Asia/Tokyo");
                assert_eq!(calc_zone(), "Asia/Tokyo");
            }
            assert_eq!(calc_zone(), "Europe/London");
        }
        assert_eq!(calc_zone(), machine_zone());
    }
}
