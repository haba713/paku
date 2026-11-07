// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the sysupgrade part of sync_prepare in src/pacman/sync.c and the warnings
// of check_literal in lib/libalpm/sync.c.

//! `-u`: adding upgrades of installed packages to the transaction.

use alpm::PkgMeta;
use alpm::resolve::Candidate;
use alpm::sysupgrade::{Outcome, sysupgrade};
use alpm::vercmp;

use crate::session::Session;
use crate::ui::msg;

/// Upgrades for installed packages not already among `targets`, with pacman's warnings.
pub fn collect<'a>(s: &'a Session, targets: &[Candidate<'a>]) -> Vec<Candidate<'a>> {
    msg::colon("Starting full system upgrade...");
    let skip: Vec<&str> = targets.iter().map(|c| c.pkg.name.as_str()).collect();
    let downgrade = s.args.sysupgrade > 1;
    let outcomes = sysupgrade(s.local.packages(), &s.sync, &skip, downgrade, &|p| ignored(s, p));
    let mut out = Vec::new();
    for o in outcomes {
        if let Some(c) = report(s, o) {
            out.push(c);
        }
    }
    out
}

/// Prints the warning for an outcome; returns the package to install, if any.
fn report<'a>(s: &Session, o: Outcome<'a>) -> Option<Candidate<'a>> {
    match o {
        Outcome::Upgrade(c) => {
            warn_downgrade(s, c);
            return Some(c);
        }
        Outcome::Ignored {
            name,
            local,
            sync,
            downgrade,
        } => {
            let kind = if downgrade { "downgrade" } else { "upgrade" };
            msg::warning(&format!("{name}: ignoring package {kind} ({local} => {sync})"));
        }
        Outcome::LocalNewer {
            name,
            local,
            repo,
            sync,
        } => msg::warning(&format!("{name}: local ({local}) is newer than {repo} ({sync})")),
        Outcome::Replaced { old, by } => msg::warning(&format!(
            "{}/{} replaces {old}; replacing packages is not supported yet",
            by.repo, by.pkg.name
        )),
    }
    None
}

fn warn_downgrade(s: &Session, c: Candidate) {
    let Some(local) = s.local.find(&c.pkg.name) else {
        return;
    };
    if vercmp(&c.pkg.version, &local.meta.version).is_lt() {
        msg::warning(&format!(
            "{}: downgrading from version {} to version {}",
            c.pkg.name, local.meta.version, c.pkg.version
        ));
    }
}

/// `IgnorePkg` / `IgnoreGroup` (including `--ignore` / `--ignoregroup`).
fn ignored(s: &Session, p: &PkgMeta) -> bool {
    s.conf.ignore_pkg.contains(&p.name) || p.groups.iter().any(|g| s.conf.ignore_group.contains(g))
}
