// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Package databases: sync (remote repositories), local (installed packages) and
//! the read-only set of packages bundled with Git for Windows.

pub mod git_provided;
pub mod local;
pub mod sync;

pub use git_provided::GitProvided;
pub use local::LocalDb;
pub use sync::SyncDb;
