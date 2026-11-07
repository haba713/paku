// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! The action log (`paku.log`), in pacman's log format:
//! `[2026-11-07T15:10:02+0200] [ALPM] installed rsync (3.5.2-1)`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use paku_platform::{LocalTime, local_time, utc_time};

use crate::error::{Result, io_err};

/// Appends one line to the log. The `[ALPM]` tag keeps pacman log tools working.
pub fn log_action(logfile: &Path, message: &str) -> Result<()> {
    if let Some(dir) = logfile.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logfile)
        .map_err(io_err(logfile))?;
    writeln!(file, "[{}] [ALPM] {message}", timestamp(now())).map_err(io_err(logfile))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// ISO 8601 local time with UTC offset, e.g. `2026-11-07T15:10:02+0200`.
fn timestamp(unix: i64) -> String {
    let local = local_time(unix);
    let offset_min = (seconds_of(&local) - seconds_of(&utc_time(unix))) / 60;
    let sign = if offset_min < 0 { '-' } else { '+' };
    let off = offset_min.abs();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{sign}{:02}{:02}",
        local.year,
        local.month,
        local.day,
        local.hour,
        local.minute,
        local.second,
        off / 60,
        off % 60
    )
}

/// Seconds since an arbitrary epoch, enough to diff two nearby calendar times.
fn seconds_of(t: &LocalTime) -> i64 {
    let days = i64::from(t.year) * 372 + i64::from(t.month) * 31 + i64::from(t.day);
    days * 86_400 + i64::from(t.hour) * 3600 + i64::from(t.minute) * 60 + i64::from(t.second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let log = dir.path().join("sub").join("paku.log");
        log_action(&log, "installed a (1-1)").unwrap();
        log_action(&log, "installed b (2-1)").unwrap();
        let text = fs::read_to_string(&log).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with("] [ALPM] installed b (2-1)"));
        assert_eq!(lines[0].find(']'), Some(25));
    }
}
