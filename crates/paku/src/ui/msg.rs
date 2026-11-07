// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! `error:`/`warning:`/`::` messages (pacman's `pm_printf` and `colon_printf`).

use super::color::colors;

/// Prints `error: <msg>` to stderr.
pub fn error(msg: &str) {
    let c = colors();
    eprintln!("{}error:{} {msg}", c.err, c.nocolor);
}

/// Prints `warning: <msg>` to stderr.
pub fn warning(msg: &str) {
    let c = colors();
    eprintln!("{}warning:{} {msg}", c.warn, c.nocolor);
}

/// Prints `:: <msg>` to stdout.
pub fn colon(msg: &str) {
    let c = colors();
    println!("{}{msg}{}", c.colon, c.nocolor);
}
