// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the HoldPkg check and display_targets / print_packages calls in
// src/pacman/remove.c and src/pacman/util.c.

//! The package list and questions before removing.

use std::fmt::Write;

use alpm::install::conflict::glob_match;
use alpm::remove::Plan;

use super::removed_size;
use crate::session::Session;
use crate::ui::color::colors;
use crate::ui::display::{humanize_size, list_display};
use crate::ui::msg;
use crate::ui::prompt::{noyes, yesno};

/// Packages matching `HoldPkg` need an extra, default-no confirmation.
pub fn holdpkg_ok(s: &Session, plan: &Plan) -> bool {
    let mut held = false;
    for p in &plan.remove {
        if s.conf.hold_pkg.iter().any(|h| glob_match(h, &p.meta.name)) {
            msg::warning(&format!("{} is designated as a HoldPkg.", p.meta.name));
            held = true;
        }
    }
    !held
        || noyes(
            "HoldPkg was found in target list. Do you want to continue?",
            s.args.noconfirm == Some(true),
        )
}

/// Prints `Packages (N) ...` and the removed size, then asks to proceed.
pub fn confirm(s: &Session, plan: &Plan) -> bool {
    let names: Vec<String> = plan
        .remove
        .iter()
        .map(|p| format!("{}-{}", p.meta.name, p.meta.version))
        .collect();
    let mut out = String::from("\n");
    list_display(&mut out, &format!("Packages ({})", names.len()), &names, s.cols);
    out.push('\n');
    let c = colors();
    let (value, unit) = humanize_size(removed_size(plan) as i64, None);
    let _ = writeln!(
        out,
        "{}{:<22}{} {value:.2} {unit}",
        c.title, "Total Removed Size:", c.nocolor
    );
    println!("{out}");
    yesno("Do you want to remove these packages?", s.args.noconfirm == Some(true))
}

/// `-p`: one line per package; `%l` of an installed package is `name-version`.
pub fn print_targets(s: &Session, plan: &Plan) {
    for p in &plan.remove {
        let m = &p.meta;
        let line = match &s.args.print_format {
            Some(fmt) => fmt
                .replace("%n", &m.name)
                .replace("%v", &m.version)
                .replace("%r", "local")
                .replace("%a", &m.arch)
                .replace("%s", "0")
                .replace("%l", &format!("{}-{}", m.name, m.version))
                .replace("%d", &m.desc),
            None => format!("{}-{}", m.name, m.version),
        };
        println!("{line}");
    }
}
