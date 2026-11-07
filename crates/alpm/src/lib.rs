// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Package management library for paku, a Rust port of pacman's libalpm.
//!
//! The library never prints or prompts; front-ends receive results and events.

pub mod checksum;
pub mod db;
pub mod deps;
pub mod download;
mod error;
pub mod install;
pub mod lock;
pub mod log;
pub mod resolve;
pub mod search;
pub mod sysupgrade;
pub mod version;

pub use error::{Error, Result};
pub use paku_formats::meta::PkgMeta;
pub use version::vercmp;
