// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of sync_trans in src/pacman/sync.c and _alpm_sync_prepare / _alpm_sync_commit
// in lib/libalpm/sync.c.

//! `-S <targets>` and `-Su`: resolve, confirm, download, verify, extract, check conflicts, install.

mod commit;
mod confirm;
mod notes;
mod replace;
mod signatures;
mod targets;
mod upgrades;

use std::path::PathBuf;

use alpm::db::git_provided::is_core_package;
use alpm::db::local::LocalPkg;
use alpm::deps::satisfies;
use alpm::download::fetch_package;
use alpm::lock::DbLock;
use alpm::resolve::{Candidate, resolve};
use paku_formats::depend::Depend;

use crate::session::Session;
use crate::ui::msg;

/// What an install transaction changes.
struct Transaction<'a> {
    /// Packages to install, dependencies first.
    plan: Vec<Candidate<'a>>,
    /// Those of `plan` to record as explicitly installed.
    explicit: Vec<Candidate<'a>>,
    /// Installed packages that `plan` replaces; removed before installing.
    removes: Vec<&'a LocalPkg>,
}

/// Runs an install transaction for the command-line targets.
pub fn run(s: &Session) -> i32 {
    let _lock = match DbLock::acquire(&s.conf.dbpath) {
        Ok(lock) => lock,
        Err(e) => return lock_error(&e),
    };
    let targets = targets::collect(s);
    if !targets.ok {
        return 1;
    }
    let mut wanted = targets.packages.clone();
    let mut replaced = Vec::new();
    if s.args.sysupgrade > 0 {
        let upgrades = upgrades::collect(s, &targets.packages);
        wanted.extend(upgrades.packages);
        replaced = upgrades.replaced;
    }
    if wanted.is_empty() {
        println!(" there is nothing to do");
        return 0;
    }
    if !s.args.print {
        println!("resolving dependencies...");
    }
    let plan = match resolve(&s.sync, &wanted, &|d| is_installed(s, d)) {
        Ok(plan) => plan,
        Err(e) => {
            msg::error("failed to prepare transaction (could not satisfy dependencies)");
            println!(":: {e}");
            return 1;
        }
    };
    let t = Transaction {
        plan,
        explicit: replace::explicit(&targets.packages, &replaced),
        removes: replace::removals(&replaced),
    };
    if !valid_arch(s, &t.plan) || !no_package_conflicts(s, &t) || !replace::deps_ok(s, &t) {
        return 1;
    }
    if s.args.print {
        confirm::print_targets(s, &t.plan);
        return 0;
    }
    if !confirm::confirm(s, &t) {
        return 1;
    }
    proceed(s, &t)
}

/// Download, then (unless `-w`) install.
fn proceed(s: &Session, t: &Transaction) -> i32 {
    let Some(files) = download(s, &t.plan) else {
        return 1;
    };
    println!("checking package integrity...");
    if !signatures::verify(s, &t.plan, &files) {
        return 1;
    }
    if s.args.downloadonly {
        return 0;
    }
    commit::install(s, t, &files)
}

/// Whether something installed (paku's packages or Git's bundle) satisfies `dep`.
/// Core packages always count as provided by Git.
fn is_installed(s: &Session, dep: &Depend) -> bool {
    is_core_package(&dep.name)
        || s.local
            .packages()
            .iter()
            .any(|p| satisfies(dep, &p.meta.name, &p.meta.version, &p.meta.provides))
        || s.git.packages().iter().any(|(n, v)| satisfies(dep, n, v, &[]))
}

fn download(s: &Session, plan: &[Candidate]) -> Option<Vec<PathBuf>> {
    msg::colon("Retrieving packages...");
    let cache = s.conf.cachedirs.first()?;
    let opts = s.download_options();
    let mut files = Vec::new();
    for c in plan {
        let servers = s
            .sync
            .iter()
            .find(|db| db.name() == c.repo)
            .map(|db| db.servers().to_vec())?;
        let filename = c.pkg.filename.as_deref().unwrap_or_default();
        match fetch_package(&servers, filename, cache, c.pkg.sha256.as_deref(), &opts) {
            Ok(got) => {
                if !got.was_cached {
                    println!(" {}-{} downloaded", c.pkg.name, c.pkg.version);
                }
                files.push(got.path);
            }
            Err(e) => {
                msg::error(&e.to_string());
                msg::error("failed to commit transaction (failed to retrieve some files)");
                return None;
            }
        }
    }
    Some(files)
}

/// Packages must be for this machine: `any`, the native architecture, or x86_64
/// (msys packages, which run under emulation on ARM64).
fn valid_arch(s: &Session, plan: &[Candidate]) -> bool {
    let bad: Vec<_> = plan
        .iter()
        .filter(|c| !["any", "x86_64", s.conf.arch.as_str()].contains(&c.pkg.arch.as_str()))
        .collect();
    for c in &bad {
        msg::error(&format!(
            "package {} does not have a valid architecture ({})",
            c.pkg.name, c.pkg.arch
        ));
    }
    bad.is_empty()
}

/// Declared `CONFLICTS` against installed packages that stay installed (pacman
/// would offer removal). Packages being replaced are removed anyway.
fn no_package_conflicts(s: &Session, t: &Transaction) -> bool {
    if !s.args.print {
        println!("looking for conflicting packages...");
    }
    let stays = |l: &LocalPkg| !t.removes.iter().any(|r| r.meta.name == l.meta.name);
    let mut ok = true;
    for c in &t.plan {
        for conflict in c.pkg.conflicts.iter().map(|d| Depend::parse(d)) {
            let hit = s.local.packages().iter().filter(|l| stays(l)).find(|l| {
                l.meta.name != c.pkg.name && satisfies(&conflict, &l.meta.name, &l.meta.version, &l.meta.provides)
            });
            if let Some(l) = hit {
                msg::error(&format!(
                    "{} and {} are in conflict; remove {} first",
                    c.pkg.name, l.meta.name, l.meta.name
                ));
                ok = false;
            }
        }
    }
    ok
}

pub(super) fn lock_error(e: &alpm::Error) -> i32 {
    msg::error("failed to init transaction (unable to lock database)");
    msg::error(&e.to_string());
    if let alpm::Error::Locked(path) = e {
        eprintln!(
            "  if you're sure a package manager is not already running, you can remove {}",
            path.display()
        );
    }
    1
}
