//! Calendar formatting for epoch times, without a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 → `(year, month, day)`. Howard Hinnant's civil_from_days.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Epoch ms → ISO 8601 in UTC with milliseconds, e.g. `2026-10-07T14:03:05.123Z`.
pub fn iso8601(ms: u64) -> String {
    let secs = ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = ((secs % 86_400) / 3600, (secs % 3600) / 60, secs % 60);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        ms % 1000
    )
}

/// A file time → epoch ms; one before 1970 reads as 0.
pub fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn iso8601_formats_epoch_millis_in_utc() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601(1_791_381_785_123), "2026-10-07T14:03:05.123Z");
        assert_eq!(iso8601(951_868_799_999), "2000-02-29T23:59:59.999Z");
    }

    #[test]
    fn file_times_convert_to_epoch_millis() {
        assert_eq!(epoch_ms(UNIX_EPOCH + Duration::from_millis(1234)), 1234);
        assert_eq!(epoch_ms(UNIX_EPOCH - Duration::from_secs(1)), 0);
    }
}
