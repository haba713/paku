// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Installing packages: extraction into a staging area, file conflict checks and
//! committing into the install prefix (role of libalpm's `add.c` and `conflict.c`).

pub mod commit;
pub mod conflict;
pub mod extract;

#[cfg(test)]
pub(crate) mod testutil;
