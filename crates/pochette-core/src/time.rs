//! Times, as Unix times in seconds, and their dates in the device's time zone.

use jiff::{Timestamp, civil, tz::TimeZone};

/// Now, in Unix time.
#[must_use]
pub fn now() -> i64 {
    Timestamp::now().as_second()
}

/// The year, month and day of `at`, in the local time zone.
#[must_use]
pub fn local(at: i64) -> (i32, i32, i32) {
    Timestamp::from_second(at)
        .map(|at| {
            let date = at.to_zoned(TimeZone::system()).date();
            (
                i32::from(date.year()),
                i32::from(date.month()),
                i32::from(date.day()),
            )
        })
        .unwrap_or_default()
}

/// The Unix time of `hour` o'clock on that day, in the local time zone.
#[must_use]
pub fn local_unix(year: i32, month: i32, day: i32, hour: i32) -> Option<i64> {
    let date = civil::Date::new(
        i16::try_from(year).ok()?,
        i8::try_from(month).ok()?,
        i8::try_from(day).ok()?,
    )
    .ok()?;
    let at = date.at(i8::try_from(hour).ok()?, 0, 0, 0);
    at.to_zoned(TimeZone::system())
        .ok()
        .map(|zoned| zoned.timestamp().as_second())
}

/// How many days `month` of `year` has.
#[must_use]
pub fn days_in_month(year: i32, month: i32) -> i32 {
    i16::try_from(year)
        .ok()
        .zip(i8::try_from(month).ok())
        .and_then(|(year, month)| civil::Date::new(year, month, 1).ok())
        .map_or(31, |date| i32::from(date.days_in_month()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_local_dates() {
        let at = local_unix(2026, 11, 24, 12).unwrap();
        assert_eq!(local(at), (2026, 11, 24));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2026, 12), 31);
    }
}
