// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the sysupgrade part of sync_prepare in src/pacman/sync.c and the warnings
// of check_literal / check_replacers in lib/libalpm/sync.c.

//! `-u`: adding upgrades and replacements of installed packages to the transaction.

use alpm::PkgMeta;
use alpm::db::local::LocalPkg;
use alpm::resolve::Candidate;
use alpm::sysupgrade::{Outcome, Policy, sysupgrade};
use alpm::vercmp;

use super::replace::Replacement;
use crate::session::Session;
use crate::ui::msg;
use crate::ui::prompt::yesno;

/// What `-u` adds to the transaction.
#[derive(Default)]
pub struct Upgrades<'a> {
    /// Other versions of installed packages, and packages replacing installed ones.
    pub packages: Vec<Candidate<'a>>,
    /// Installed packages that a package in the transaction replaces.
    pub replaced: Vec<Replacement<'a>>,
}

/// Upgrades for installed packages not already among `targets`, with pacman's
/// warnings and its `Replace a with repo/b?` question.
pub fn collect<'a>(s: &'a Session, targets: &[Candidate<'a>]) -> Upgrades<'a> {
    if !s.args.print {
        msg::colon("Starting full system upgrade...");
    }
    let skip: Vec<&str> = targets.iter().map(|c| c.pkg.name.as_str()).collect();
    let policy = Policy {
        downgrade: s.args.sysupgrade > 1,
        ignored: &|p| ignored(s, p),
        replace: &|old, by| confirm_replace(s, old, by),
    };
    let mut out = Upgrades::default();
    for o in sysupgrade(s.local.packages(), &s.sync, &skip, &policy) {
        apply(s, o, targets, &mut out);
    }
    out
}

/// Records an outcome in `out`, or prints the warning that explains why not.
fn apply<'a>(s: &Session, o: Outcome<'a>, targets: &[Candidate<'a>], out: &mut Upgrades<'a>) {
    match o {
        Outcome::Upgrade(c) => {
            warn_downgrade(s, c);
            out.packages.push(c);
        }
        Outcome::Replaced { old, by } => {
            let planned = |c: &Candidate| c.pkg.name == by.pkg.name;
            if !targets.iter().any(planned) && !out.packages.iter().any(planned) {
                out.packages.push(by);
            }
            out.replaced.push(Replacement { old, by });
        }
        Outcome::IgnoredReplacement { old, by } => msg::warning(&format!(
            "ignoring package replacement ({}-{} => {}-{})",
            old.meta.name, old.meta.version, by.pkg.name, by.pkg.version
        )),
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
    }
}

/// `-p` only prints, so it assumes yes without asking (as pacman does).
fn confirm_replace(s: &Session, old: &LocalPkg, by: &Candidate) -> bool {
    if s.args.print {
        return true;
    }
    let question = format!("Replace {} with {}/{}?", old.meta.name, by.repo, by.pkg.name);
    yesno(&question, s.args.noconfirm == Some(true))
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
