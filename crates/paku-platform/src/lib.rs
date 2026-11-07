// SPDX-License-Identifier: MIT

//! Host platform helpers for paku: native architecture, HTTP over WinHTTP,
//! temporary directories, terminal detection (including Git Bash's mintty)
//! and local time conversion.

mod arch;
pub mod http;
mod tempdir;
mod term;
mod time;

pub use arch::{HostArch, compiled_arch, native_arch};
pub use tempdir::TempDir;
pub use term::{Stream, is_terminal};
pub use time::{LocalTime, local_time, utc_time};

#[cfg(windows)]
mod ffi;
