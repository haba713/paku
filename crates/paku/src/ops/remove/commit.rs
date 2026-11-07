// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_remove_packages / _alpm_remove_single_package in lib/libalpm/remove.c.

//! Deleting each package's files and database entry.

use std::collections::HashSet;

use alpm::db::local::{LocalPkg, remove_entry};
use alpm::log::log_action;
use alpm::remove::Plan;
use alpm::remove::files::remove_files;

use crate::session::Session;
use crate::ui::msg;

/// Removes the planned packages in order. Returns the exit code.
pub fn remove(s: &Session, plan: &Plan) -> i32 {
    msg::colon("Processing package changes...");
    let layout = s.layout();
    let keep_dirs = remaining_dirs(s, plan);
    let total = plan.remove.len();
    for (i, p) in plan.remove.iter().enumerate() {
        println!("({}/{total}) removing {}", i + 1, p.meta.name);
        if !s.args.dbonly {
            report(remove_files(&layout, &p.files, &keep_dirs));
        }
        if let Err(e) = remove_entry(&s.conf.dbpath, &p.meta.name, &p.meta.version) {
            msg::error(&format!(
                "could not remove database entry {}-{}: {e}",
                p.meta.name, p.meta.version
            ));
            return 1;
        }
        if let Err(e) = log_action(
            &s.conf.logfile,
            &format!("removed {} ({})", p.meta.name, p.meta.version),
        ) {
            msg::warning(&e.to_string());
        }
    }
    0
}

/// Directories owned by packages that stay installed (lowercase, as in file lists).
fn remaining_dirs(s: &Session, plan: &Plan) -> HashSet<String> {
    let removed = |p: &LocalPkg| plan.remove.iter().any(|r| r.meta.name == p.meta.name);
    s.local
        .packages()
        .iter()
        .filter(|p| !removed(p))
        .flat_map(|p| p.files.iter())
        .filter(|f| f.ends_with('/'))
        .map(|f| f.to_ascii_lowercase())
        .collect()
}

fn report(r: alpm::remove::files::Removed) {
    for (path, err) in &r.failed {
        msg::error(&format!("cannot remove {} ({err})", path.display()));
    }
    for f in &r.leftovers {
        msg::warning(&format!(
            "{} is in use; delete it after closing the program",
            f.display()
        ));
    }
}
