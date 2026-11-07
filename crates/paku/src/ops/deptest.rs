// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/pacman/deptest.c.

//! `-T`: prints the dependency strings that are not satisfied; exit 127 if any.

use alpm::deps::satisfies;
use paku_formats::depend::Depend;

use crate::session::Session;

/// Runs `-T`.
pub fn run(s: &mut Session) -> i32 {
    let missing: Vec<&String> = s
        .args
        .targets
        .iter()
        .filter(|t| !is_satisfied(s, &Depend::parse(t)))
        .collect();
    for target in &missing {
        println!("{target}");
    }
    if missing.is_empty() { 0 } else { 127 }
}

fn is_satisfied(s: &Session, dep: &Depend) -> bool {
    let local = s
        .local
        .packages()
        .iter()
        .any(|p| satisfies(dep, &p.meta.name, &p.meta.version, &p.meta.provides));
    local
        || s.git
            .packages()
            .iter()
            .any(|(name, version)| satisfies(dep, name, version, &[]))
}
