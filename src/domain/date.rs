//! Minimal proleptic-Gregorian date math for the timeline axis, expressed in
//! days since the Unix epoch (1970-01-01 = 0). Uses Howard Hinnant's
//! `days_from_civil`/`civil_from_days` algorithms so no date crate is needed.
//! Only whole days matter for the timeline, so time-of-day and zone offsets are
//! ignored.

use std::time::{SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 for the given civil date. `month` is 1..=12.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

/// Civil date `(year, month, day)` for a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719468;
    let era = (if days >= 0 { days } else { days - 146096 }) / 146097;
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Today as days since 1970-01-01, in local terms approximated by UTC (whole
/// days only, so the off-by-a-few-hours zone difference never shifts the axis).
pub fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| (elapsed.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

/// Parses the leading `YYYY-MM-DD` of an ISO-8601 timestamp into days since the
/// epoch. Returns `None` for malformed input. Greenhopper/agile encode zone
/// offsets without a colon (`+0200`), so only the date prefix is read.
pub fn iso_to_days(iso: &str) -> Option<i64> {
    let date = iso.get(..10)?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if (1..=12).contains(&month) && (1..=31).contains(&day) {
        Some(days_from_civil(year, month, day))
    } else {
        None
    }
}

/// The first day of the month after the month containing `(year, month)`.
pub fn next_month(year: i64, month: u32) -> (i64, u32) {
    if month >= 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, days_from_civil, iso_to_days, next_month};

    #[test]
    fn epoch_and_known_dates_map_to_expected_day_counts() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        // 2000-03-01 is 11017 days after the epoch (a well-known reference).
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }

    #[test]
    fn civil_round_trips_through_days() {
        for &(y, m, d) in &[(1970, 1, 1), (2024, 2, 29), (2026, 6, 11), (1999, 12, 31)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
    }

    #[test]
    fn iso_prefix_parses_and_ignores_time_and_zone() {
        assert_eq!(
            iso_to_days("2026-06-03T10:30:55+0200"),
            Some(days_from_civil(2026, 6, 3))
        );
        assert_eq!(iso_to_days("not-a-date"), None);
        assert_eq!(iso_to_days("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn next_month_rolls_over_the_year() {
        assert_eq!(next_month(2026, 6), (2026, 7));
        assert_eq!(next_month(2026, 12), (2027, 1));
    }
}
