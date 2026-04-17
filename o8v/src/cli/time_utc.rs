// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Unix-seconds → UTC civil-date formatting.
//!
//! No dependency on chrono or a system TZ database. Uses Howard Hinnant's
//! `civil_from_days` algorithm (public domain), which correctly handles
//! leap years including the non-leap centuries (2100, 2200, 2300).

/// Format Unix seconds as `YYYY-MM-DD HH:MM:SS UTC`.
pub(crate) fn format_unix_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
}

/// Howard Hinnant's civil_from_days — unix-epoch days (1970-01-01 = 0) to
/// (year, month, day). Correct for all proleptic Gregorian dates in i64 range.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + i64::from(m <= 2);
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::format_unix_utc;

    #[test]
    fn unix_epoch_is_1970_01_01() {
        assert_eq!(format_unix_utc(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn one_day_after_epoch() {
        assert_eq!(format_unix_utc(86_400), "1970-01-02 00:00:00 UTC");
    }

    #[test]
    fn one_second_before_y2k() {
        // 1999-12-31 23:59:59 UTC
        assert_eq!(format_unix_utc(946_684_799), "1999-12-31 23:59:59 UTC");
    }

    #[test]
    fn y2k_exact() {
        assert_eq!(format_unix_utc(946_684_800), "2000-01-01 00:00:00 UTC");
    }

    #[test]
    fn year_2020_new_year() {
        assert_eq!(format_unix_utc(1_577_836_800), "2020-01-01 00:00:00 UTC");
    }

    #[test]
    fn leap_day_feb_29_2024_noon() {
        // 2024-02-29 12:00:00 UTC = 1709208000 — validates leap-year Feb 29 exists
        assert_eq!(format_unix_utc(1_709_208_000), "2024-02-29 12:00:00 UTC");
    }

    #[test]
    fn non_leap_century_2100_feb_28_is_last_february_day() {
        // 2100-02-28 00:00:00 UTC = 4107456000
        assert_eq!(format_unix_utc(4_107_456_000), "2100-02-28 00:00:00 UTC");
    }

    #[test]
    fn non_leap_century_2100_skips_feb_29() {
        // The second after 2100-02-28 23:59:59 must roll to 2100-03-01, not Feb 29.
        // 2100-02-28 00:00:00 UTC + 1 day = 2100-03-01 00:00:00 UTC
        assert_eq!(
            format_unix_utc(4_107_456_000 + 86_400),
            "2100-03-01 00:00:00 UTC"
        );
    }

    #[test]
    fn year_2400_is_leap_century() {
        // 2400 IS a leap year (divisible by 400). Feb 29 must exist.
        // 2400-02-29 00:00:00 UTC — compute:
        // 2400-01-01 is 430 years after 1970-01-01.
        // This test pins the behavior: if civil_from_days is wrong on 400-year
        // cycles, this will fail.
        let text = format_unix_utc(13_574_563_200);
        assert_eq!(text, "2400-02-29 00:00:00 UTC", "got: {text}");
    }

    #[test]
    fn hour_minute_second_fields_are_correct() {
        // 2023-06-15 13:45:23 UTC = 1686836723
        assert_eq!(format_unix_utc(1_686_836_723), "2023-06-15 13:45:23 UTC");
    }

    #[test]
    fn midnight_rollover() {
        // 2023-01-01 23:59:59 UTC → next second 2023-01-02 00:00:00 UTC
        assert_eq!(format_unix_utc(1_672_617_599), "2023-01-01 23:59:59 UTC");
        assert_eq!(format_unix_utc(1_672_617_600), "2023-01-02 00:00:00 UTC");
    }

    #[test]
    fn year_rollover() {
        // 2023-12-31 23:59:59 UTC → 2024-01-01 00:00:00 UTC
        assert_eq!(format_unix_utc(1_704_067_199), "2023-12-31 23:59:59 UTC");
        assert_eq!(format_unix_utc(1_704_067_200), "2024-01-01 00:00:00 UTC");
    }
}
