// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of pacman_remove / remove_target in src/pacman/remove.c.

//! `-R <targets>`: check dependencies, confirm, delete files, update the database.

mod commit;
mod confirm;

use alpm::db::git_provided::is_core_package;
use alpm::db::local::LocalPkg;
use alpm::deps::satisfies;
use alpm::lock::DbLock;
use alpm::remove::{Flags, Missing, Plan, prepare};
use paku_formats::depend::Depend;

use crate::session::Session;
use crate::ui::msg;

/// Runs a removal transaction for the command-line targets.
pub fn run(s: &Session) -> i32 {
    if s.args.targets.is_empty() {
        msg::error("no targets specified (use -h for help)");
        return 1;
    }
    let _lock = match DbLock::acquire(&s.conf.dbpath) {
        Ok(lock) => lock,
        Err(e) => return super::install::lock_error(&e),
    };
    let Some(targets) = collect(s) else {
        return 1;
    };
    if s.args.nodeps < 2 && !s.args.print {
        println!("checking dependencies...");
    }
    let plan = match prepare(s.local.packages(), targets, flags(s), &|d| provided(s, d)) {
        Ok(plan) => plan,
        Err(missing) => return broken(&missing),
    };
    for (pkg, opt) in plan.optdepends.iter().filter(|_| !s.args.print) {
        msg::colon(&format!("{pkg} optionally requires {opt}"));
    }
    if plan.remove.is_empty() {
        println!(" there is nothing to do");
        return 0;
    }
    if s.args.print {
        confirm::print_targets(s, &plan);
        return 0;
    }
    if !confirm::holdpkg_ok(s, &plan) || !confirm::confirm(s, &plan) {
        return 1;
    }
    commit::remove(s, &plan)
}

/// Port of `remove_target`: a package name (optionally `local/name`) or a group.
fn collect(s: &Session) -> Option<Vec<&LocalPkg>> {
    let mut out: Vec<&LocalPkg> = Vec::new();
    let mut ok = true;
    for target in &s.args.targets {
        let name = target.strip_prefix("local/").unwrap_or(target);
        let found: Vec<&LocalPkg> = match s.local.find(name) {
            Some(p) => vec![p],
            None => s
                .local
                .packages()
                .iter()
                .filter(|p| p.meta.groups.iter().any(|g| g == name))
                .collect(),
        };
        if found.is_empty() {
            not_found(s, name);
            ok = false;
        }
        for p in found {
            if !out.iter().any(|o| o.meta.name == p.meta.name) {
                out.push(p);
            }
        }
    }
    ok.then_some(out)
}

fn not_found(s: &Session, name: &str) {
    if is_core_package(name) || s.git.version_of(name).is_some() {
        msg::error(&format!(
            "{name} is part of Git for Windows and cannot be removed with paku"
        ));
    } else {
        msg::error(&format!("target not found: {name}"));
    }
}

fn flags(s: &Session) -> Flags {
    Flags {
        cascade: s.args.cascade,
        recursive: s.args.recursive,
        unneeded: s.args.unneeded,
        nodeps: s.args.nodeps,
    }
}

/// Dependencies satisfied by Git for Windows (its bundle and core packages).
fn provided(s: &Session, dep: &Depend) -> bool {
    is_core_package(&dep.name) || s.git.packages().iter().any(|(n, v)| satisfies(dep, n, v, &[]))
}

fn broken(missing: &[Missing]) -> i32 {
    msg::error("failed to prepare transaction (could not satisfy dependencies)");
    for m in missing {
        msg::colon(&format!(
            "removing {} breaks dependency '{}' required by {}",
            m.causing, m.depend, m.target
        ));
    }
    1
}

/// Sum of installed sizes, for the confirmation.
fn removed_size(plan: &Plan) -> u64 {
    plan.remove.iter().map(|p| p.meta.isize).sum()
}
