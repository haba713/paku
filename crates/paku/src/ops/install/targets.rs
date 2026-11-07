// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of process_target / process_targname / process_group in src/pacman/sync.c.

//! Turning command-line targets into sync packages.

use alpm::db::SyncDb;
use alpm::db::git_provided::is_core_package;
use alpm::resolve::{Candidate, find_satisfier};
use alpm::vercmp;
use paku_formats::depend::Depend;

use super::super::split_repo;
use crate::session::Session;
use crate::ui::msg;

/// Explicitly requested packages; `ok` is false if any target was invalid.
pub struct Targets<'a> {
    pub packages: Vec<Candidate<'a>>,
    pub ok: bool,
}

/// Resolves every target. Errors are printed; skipped targets print a warning.
pub fn collect<'a>(s: &'a Session) -> Targets<'a> {
    let mut out = Targets {
        packages: Vec::new(),
        ok: true,
    };
    for target in &s.args.targets {
        match lookup(&s.sync, target) {
            Lookup::Found(list) => {
                for c in list {
                    if keep(s, c) && !out.packages.iter().any(|p| p.pkg.name == c.pkg.name) {
                        out.packages.push(c);
                    }
                }
            }
            Lookup::NotFound => {
                msg::error(&format!("target not found: {target}"));
                out.ok = false;
            }
        }
    }
    out.ok &= out.packages.iter().all(|c| allowed(c.pkg.name.as_str()));
    out
}

enum Lookup<'a> {
    Found(Vec<Candidate<'a>>),
    NotFound,
}

/// `repo/name`, `name`, `name>=ver`, a provided name, or a group.
fn lookup<'a>(dbs: &'a [SyncDb], target: &str) -> Lookup<'a> {
    let (repo, spec) = split_repo(target);
    let scoped: &[SyncDb] = match repo {
        Some(r) => match dbs.iter().position(|db| db.name() == r) {
            Some(i) => &dbs[i..=i],
            None => &[],
        },
        None => dbs,
    };
    if let Some(c) = find_satisfier(scoped, &Depend::parse(spec)) {
        return Lookup::Found(vec![c]);
    }
    let group: Vec<Candidate<'a>> = scoped
        .iter()
        .flat_map(|db| db.packages().iter().map(move |pkg| Candidate { repo: db.name(), pkg }))
        .filter(|c| c.pkg.groups.iter().any(|g| g == spec))
        .collect();
    if group.is_empty() {
        Lookup::NotFound
    } else {
        Lookup::Found(group)
    }
}

/// Core packages come only from Git for Windows; installing them would break it.
fn allowed(name: &str) -> bool {
    if is_core_package(name) {
        msg::error(&format!(
            "{name} is part of Git for Windows and cannot be installed with paku"
        ));
        return false;
    }
    true
}

/// Applies `--needed`, `IgnorePkg` and the Git-provided rule; prints why a target is skipped.
fn keep(s: &Session, c: Candidate) -> bool {
    let (name, version) = (&c.pkg.name, &c.pkg.version);
    if s.conf.ignore_pkg.contains(name) {
        msg::warning(&format!("skipping target: {name} (IgnorePkg)"));
        return false;
    }
    if let Some(local) = s.local.find(name) {
        if vercmp(&local.meta.version, version).is_eq() {
            if s.args.needed {
                msg::warning(&format!("{name}-{version} is up to date -- skipping"));
                return false;
            }
            msg::warning(&format!("{name}-{version} is up to date -- reinstalling"));
        }
        return true;
    }
    if let Some(git_version) = s.git.version_of(name) {
        msg::warning(&format!(
            "{name}-{git_version} is provided by Git for Windows -- skipping"
        ));
        return false;
    }
    true
}
