// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Output color strings (pacman's `colstr_t`).

use std::sync::OnceLock;

const NOCOLOR: &str = "\x1b[0m";
const BOLD: &str = "\x1b[0;1m";
const BOLDRED: &str = "\x1b[1;31m";
const BOLDGREEN: &str = "\x1b[1;32m";
const BOLDYELLOW: &str = "\x1b[1;33m";
const BOLDBLUE: &str = "\x1b[1;34m";
const BOLDMAGENTA: &str = "\x1b[1;35m";
const BOLDCYAN: &str = "\x1b[1;36m";

/// Escape sequences for each kind of output; all empty when colors are off.
#[derive(Debug, Clone, Copy)]
pub struct ColStr {
    pub colon: &'static str,
    pub title: &'static str,
    pub repo: &'static str,
    pub version: &'static str,
    pub groups: &'static str,
    pub meta: &'static str,
    pub warn: &'static str,
    pub err: &'static str,
    pub nocolor: &'static str,
}

impl ColStr {
    /// Color strings for enabled or disabled colors.
    pub const fn new(on: bool) -> Self {
        if !on {
            return ColStr {
                colon: ":: ",
                title: "",
                repo: "",
                version: "",
                groups: "",
                meta: "",
                warn: "",
                err: "",
                nocolor: "",
            };
        }
        ColStr {
            colon: concat!("\x1b[1;34m", "::", "\x1b[0;1m", " "),
            title: BOLD,
            repo: BOLDMAGENTA,
            version: BOLDGREEN,
            groups: BOLDBLUE,
            meta: BOLDCYAN,
            warn: BOLDYELLOW,
            err: BOLDRED,
            nocolor: NOCOLOR,
        }
    }
}

static COLORS: OnceLock<ColStr> = OnceLock::new();

/// Enables or disables colors for the rest of the run (first call wins).
pub fn init(on: bool) {
    let _ = COLORS.set(ColStr::new(on));
}

/// Current color strings (colors off until [`init`] is called).
pub fn colors() -> &'static ColStr {
    COLORS.get_or_init(|| ColStr::new(false))
}
