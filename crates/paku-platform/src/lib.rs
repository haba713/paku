// SPDX-License-Identifier: GPL-2.0-or-later

//! Host platform helpers for paku: native architecture, HTTP over WinHTTP,
//! RSA signature verification over CNG, temporary directories, terminal
//! detection (including Git Bash's mintty) and local time conversion.

mod arch;
pub mod http;
pub mod rsa;
mod tempdir;
mod term;
mod time;

pub use arch::{HostArch, compiled_arch, native_arch};
pub use tempdir::TempDir;
pub use term::{Stream, is_terminal};
pub use time::{LocalTime, local_time, utc_time};

#[cfg(windows)]
mod ffi;
