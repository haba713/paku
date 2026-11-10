// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the `removes` handling in check_replacers / _alpm_sync_prepare in
// lib/libalpm/sync.c.

//! `Replaces`: installed packages that a package in the transaction supersedes.
//! They are removed right before the new packages are installed.

use alpm::PkgMeta;
use alpm::db::local::{LocalPkg, Reason};
use alpm::remove::broken_by_replacement;
use alpm::resolve::Candidate;

use super::super::remove;
use super::Transaction;
use crate::session::Session;

/// An installed package and the sync package that replaces it.
#[derive(Clone, Copy)]
pub struct Replacement<'a> {
    pub old: &'a LocalPkg,
    pub by: Candidate<'a>,
}

/// The installed packages to remove, each once.
pub fn removals<'a>(replaced: &[Replacement<'a>]) -> Vec<&'a LocalPkg> {
    let mut out: Vec<&LocalPkg> = Vec::new();
    for r in replaced {
        if !out.iter().any(|p| p.meta.name == r.old.meta.name) {
            out.push(r.old);
        }
    }
    out
}

/// Packages recorded as explicitly installed: the command-line targets, plus
/// replacers of explicitly installed packages (a replacer inherits the reason).
pub fn explicit<'a>(targets: &[Candidate<'a>], replaced: &[Replacement<'a>]) -> Vec<Candidate<'a>> {
    let inherited = replaced
        .iter()
        .filter(|r| r.old.reason == Reason::Explicit)
        .map(|r| r.by);
    targets.iter().copied().chain(inherited).collect()
}

/// Checks that nothing left installed depends on a removed package, unless the
/// transaction's packages provide it. `-dd` skips the check.
pub fn deps_ok(s: &Session, t: &Transaction) -> bool {
    if t.removes.is_empty() || s.args.nodeps > 1 {
        return true;
    }
    let added: Vec<&PkgMeta> = t.plan.iter().map(|c| c.pkg).collect();
    let missing = broken_by_replacement(s.local.packages(), &t.removes, &added, &|d| remove::provided(s, d));
    if missing.is_empty() {
        return true;
    }
    remove::broken(&missing);
    false
}
