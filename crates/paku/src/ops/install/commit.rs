// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_sync_commit / _alpm_upgrade_packages in lib/libalpm.

//! Extracting, checking file conflicts and committing downloaded packages.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use alpm::db::local::{NewEntry, Reason, write_entry};
use alpm::install::commit::commit;
use alpm::install::conflict::{Conflict, PlannedFiles, find_conflicts};
use alpm::install::extract::{Extracted, extract};
use alpm::log::log_action;
use alpm::resolve::Candidate;
use alpm::vercmp;
use paku_layout::Layout;
use paku_platform::TempDir;

use super::notes;
use crate::session::Session;
use crate::ui::msg;

/// Stages every package, checks file conflicts, then installs them in order.
pub fn install(s: &Session, plan: &[Candidate], files: &[PathBuf], explicit: &[Candidate]) -> i32 {
    let layout = s.layout();
    let staging = match TempDir::new_in(&s.conf.prefix, ".paku-staging-") {
        Ok(t) => t,
        Err(e) => {
            msg::error(&format!("could not create staging directory: {e}"));
            return 1;
        }
    };
    println!("loading package files...");
    let Some(extracted) = stage_all(&layout, files, staging.path()) else {
        return 1;
    };
    println!("checking for file conflicts...");
    if !conflicts_ok(s, &layout, plan, &extracted) {
        return 1;
    }
    msg::colon("Processing package changes...");
    for (i, (c, x)) in plan.iter().zip(&extracted).enumerate() {
        println!("({}/{}) {} {}", i + 1, plan.len(), action(s, c), c.pkg.name);
        let is_explicit = explicit.iter().any(|e| e.pkg.name == c.pkg.name);
        if let Err(e) = install_one(s, &layout, &staging.path().join(i.to_string()), c, x, is_explicit) {
            msg::error(&format!("failed to install {}: {e}", c.pkg.name));
            return 1;
        }
    }
    after_install(s, plan, &extracted);
    0
}

fn stage_all(layout: &Layout, files: &[PathBuf], staging: &Path) -> Option<Vec<Extracted>> {
    let mut out = Vec::new();
    for (i, file) in files.iter().enumerate() {
        match extract(file, layout, &staging.join(i.to_string())) {
            Ok(x) => out.push(x),
            Err(e) => {
                msg::error(&format!("{}: {e}", file.display()));
                return None;
            }
        }
    }
    Some(out)
}

fn conflicts_ok(s: &Session, layout: &Layout, plan: &[Candidate], extracted: &[Extracted]) -> bool {
    let planned: Vec<PlannedFiles> = plan
        .iter()
        .zip(extracted)
        .map(|(c, x)| PlannedFiles {
            name: &c.pkg.name,
            files: &x.files,
        })
        .collect();
    let conflicts = find_conflicts(&planned, &s.local, layout, &s.args.overwrite);
    if conflicts.is_empty() {
        return true;
    }
    msg::error("failed to commit transaction (conflicting files)");
    for c in &conflicts {
        println!("{}", describe(s, c));
    }
    println!("Errors occurred, no packages were upgraded.");
    false
}

fn describe(s: &Session, c: &Conflict) -> String {
    let path = s
        .layout()
        .install_path(&c.path)
        .display()
        .to_string()
        .replace('\\', "/");
    match &c.owner {
        Some(owner) => format!("{}: {path} exists in both '{}' and '{owner}'", c.package, c.package),
        None => format!("{}: {path} exists in filesystem", c.package),
    }
}

fn install_one(
    s: &Session,
    layout: &Layout,
    staging: &Path,
    c: &Candidate,
    x: &Extracted,
    is_explicit: bool,
) -> alpm::Result<()> {
    let old = s.local.find(&c.pkg.name);
    let old_files = old.map(|o| o.files.clone()).unwrap_or_default();
    let done = commit(staging, layout, &x.files, &old_files)?;
    let entry = NewEntry {
        meta: &x.meta,
        reason: reason(s, old.map(|o| o.reason), is_explicit),
        install_date: now(),
        files: &x.files,
        mtree: x.mtree.as_deref(),
        install: x.install.as_deref(),
    };
    write_entry(&s.conf.dbpath, old.map(|o| o.meta.version.as_str()), entry)?;
    log_action(&s.conf.logfile, &log_line(s, c))?;
    warn_extras(s, c, x, &done.leftovers);
    Ok(())
}

/// `--asdeps`/`--asexplicit` win; reinstalls keep their reason; new dependencies are deps.
fn reason(s: &Session, old: Option<Reason>, is_explicit: bool) -> Reason {
    if s.args.asdeps {
        return Reason::Depend;
    }
    if s.args.asexplicit {
        return Reason::Explicit;
    }
    match (old, is_explicit) {
        (Some(r), _) => r,
        (None, true) => Reason::Explicit,
        (None, false) => Reason::Depend,
    }
}

fn action(s: &Session, c: &Candidate) -> &'static str {
    match s.local.find(&c.pkg.name) {
        None => "installing",
        Some(old) if vercmp(&old.meta.version, &c.pkg.version).is_eq() => "reinstalling",
        Some(old) if vercmp(&old.meta.version, &c.pkg.version).is_gt() => "downgrading",
        Some(_) => "upgrading",
    }
}

fn log_line(s: &Session, c: &Candidate) -> String {
    let (name, new) = (&c.pkg.name, &c.pkg.version);
    match s.local.find(name) {
        None => format!("installed {name} ({new})"),
        Some(old) if old.meta.version == *new => format!("reinstalled {name} ({new})"),
        Some(old) => format!("upgraded {name} ({} -> {new})", old.meta.version),
    }
}

fn warn_extras(s: &Session, c: &Candidate, x: &Extracted, leftovers: &[PathBuf]) {
    for link in &x.skipped_links {
        msg::warning(&format!("{}: skipped link {link} (target not in package)", c.pkg.name));
    }
    if x.install.is_some() && !s.args.noscriptlet {
        msg::warning(&format!("{}: install script not run (not supported yet)", c.pkg.name));
    }
    for f in leftovers {
        msg::warning(&format!(
            "{} is in use; delete it after closing the program",
            f.display()
        ));
    }
}

fn after_install(s: &Session, plan: &[Candidate], extracted: &[Extracted]) {
    for (c, x) in plan.iter().zip(extracted) {
        if s.local.find(&c.pkg.name).is_none() {
            notes::optdepends(&x.meta);
        }
        notes::msys_caveat(c.repo, &c.pkg.name, &x.files);
    }
    notes::path_hint(&s.layout().install_path("bin"), &s.conf.home);
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
