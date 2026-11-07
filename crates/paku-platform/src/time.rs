// SPDX-License-Identifier: MIT

//! Unix timestamps to local calendar time, without a timezone database.

/// Broken-down local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1-12
    pub month: u32,
    /// 1-31
    pub day: u32,
    /// 0 = Sunday
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Converts a Unix timestamp to local time using the Windows timezone settings
/// (UTC on other platforms or if conversion fails).
pub fn local_time(unix: i64) -> LocalTime {
    local_impl(unix).unwrap_or_else(|| utc_time(unix))
}

/// Converts a Unix timestamp to UTC calendar time.
pub fn utc_time(unix: i64) -> LocalTime {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    LocalTime {
        year,
        month,
        day,
        weekday: (days + 4).rem_euclid(7) as u32,
        hour: secs / 3600,
        minute: secs / 60 % 60,
        second: secs % 60,
    }
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian
/// (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

#[cfg(windows)]
fn local_impl(unix: i64) -> Option<LocalTime> {
    use crate::ffi::{FILETIME, FileTimeToSystemTime, SYSTEMTIME, SystemTimeToTzSpecificLocalTime};

    // FILETIME counts 100 ns intervals since 1601-01-01
    let ticks = (unix.checked_add(11_644_473_600)?).checked_mul(10_000_000)?;
    let ticks = u64::try_from(ticks).ok()?;
    let ft = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    // SAFETY: all pointers reference valid, initialized stack values; a null
    // timezone means "current timezone".
    let ok = unsafe {
        FileTimeToSystemTime(&ft, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0
    };
    ok.then(|| LocalTime {
        year: i32::from(local.wYear),
        month: u32::from(local.wMonth),
        day: u32::from(local.wDay),
        weekday: u32::from(local.wDayOfWeek),
        hour: u32::from(local.wHour),
        minute: u32::from(local.wMinute),
        second: u32::from(local.wSecond),
    })
}

#[cfg(not(windows))]
fn local_impl(_unix: i64) -> Option<LocalTime> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_conversion() {
        // 2025-07-19 04:35:04 UTC, a Saturday
        let t = utc_time(1_752_899_704);
        assert_eq!((t.year, t.month, t.day, t.weekday), (2025, 7, 19, 6));
        assert_eq!((t.hour, t.minute, t.second), (4, 35, 4));
        assert_eq!(utc_time(0).weekday, 4);
        assert_eq!(
            utc_time(951_782_400),
            LocalTime {
                year: 2000,
                month: 2,
                day: 29,
                weekday: 2,
                hour: 0,
                minute: 0,
                second: 0
            }
        );
    }

    #[test]
    fn local_time_is_close_to_utc() {
        let utc = utc_time(1_752_899_704);
        let local = local_time(1_752_899_704);
        assert_eq!(local.year, utc.year);
        assert!(local.day.abs_diff(utc.day) <= 1);
    }
}
