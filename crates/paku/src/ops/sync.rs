// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/pacman/sync.c (database refresh and the query-style -S options).

//! `-S`: refresh (`-y`), search (`-s`), info (`-i`), list (`-l`), groups (`-g`).

use std::fmt::Write;

use alpm::PkgMeta;
use alpm::db::SyncDb;
use alpm::download::Fetched;
use alpm::search::Search;

use super::split_repo;
use crate::session::Session;
use crate::ui::color::colors;
use crate::ui::display::indentprint;
use crate::ui::info::{Origin, dump_pkg_full};
use crate::ui::msg;

/// Runs `-S` with its sub-options in pacman's order.
pub fn run(s: &mut Session) -> i32 {
    if s.args.clean > 0 {
        msg::error("--clean is not implemented yet");
        return 1;
    }
    if s.args.refresh > 0 && refresh(s) != 0 {
        return 1;
    }
    let a = &s.args;
    let query = a.search || a.groups > 0 || a.info > 0 || a.list;
    if !query && a.targets.is_empty() && a.sysupgrade == 0 {
        if a.refresh > 0 {
            return 0;
        }
        msg::error("no targets specified (use -h for help)");
        return 1;
    }
    if !s.load_sync_dbs() {
        return 1;
    }
    let a = &s.args;
    if a.search {
        return search(s);
    }
    if a.groups > 0 {
        return groups(s);
    }
    if a.info > 0 {
        return info(s);
    }
    if a.list {
        return list(s);
    }
    if a.sysupgrade > 0 {
        msg::error("--sysupgrade is not implemented yet");
        return 1;
    }
    super::install::run(s)
}

/// Downloads fresh databases (`-y`, `-yy` forces).
fn refresh(s: &Session) -> i32 {
    msg::colon("Synchronizing package databases...");
    let force = s.args.refresh > 1;
    let opts = s.download_options();
    let mut failed = false;
    for db in &s.sync {
        match db.refresh(&s.conf.dbpath, force, &opts) {
            Ok(Fetched::UpToDate) => println!(" {} is up to date", db.name()),
            Ok(Fetched::Updated) => println!(" {} downloaded", db.name()),
            Err(e) => {
                msg::error(&e.to_string());
                failed = true;
            }
        }
    }
    if failed {
        msg::error("failed to synchronize all databases");
        return 1;
    }
    0
}

fn search(s: &Session) -> i32 {
    let searcher = match Search::new(&s.args.targets) {
        Ok(x) => x,
        Err(e) => {
            msg::error(&e.to_string());
            return 1;
        }
    };
    let mut out = String::new();
    for db in &s.sync {
        for pkg in db.packages().iter().filter(|p| searcher.matches(p)) {
            search_entry(&mut out, s, db.name(), pkg);
        }
    }
    print!("{out}");
    i32::from(out.is_empty())
}

/// One `-Ss` result: `repo/name version (groups) [installed]` plus description.
fn search_entry(out: &mut String, s: &Session, repo: &str, pkg: &PkgMeta) {
    if s.args.quiet {
        out.push_str(&pkg.name);
        out.push('\n');
        return;
    }
    let c = colors();
    let _ = write!(
        out,
        "{}{repo}/{}{} {}{}{}",
        c.repo, c.title, pkg.name, c.version, pkg.version, c.nocolor
    );
    if !pkg.groups.is_empty() {
        let _ = write!(out, " {}({}){}", c.groups, pkg.groups.join(" "), c.nocolor);
    }
    installed_marker(out, s, pkg);
    out.push_str("\n    ");
    indentprint(out, &pkg.desc, 4, s.cols);
    out.push('\n');
}

/// ` [installed]` or ` [installed: <local version>]`.
fn installed_marker(out: &mut String, s: &Session, pkg: &PkgMeta) {
    let Some(local) = s.installed_version(&pkg.name) else {
        return;
    };
    let c = colors();
    if local == pkg.version {
        let _ = write!(out, " {}[installed]{}", c.meta, c.nocolor);
    } else {
        let _ = write!(out, " {}[installed: {local}]{}", c.meta, c.nocolor);
    }
}

fn info(s: &Session) -> i32 {
    let extra = s.args.info > 1;
    let mut out = String::new();
    if s.args.targets.is_empty() {
        for db in &s.sync {
            for pkg in db.packages() {
                dump_pkg_full(&mut out, pkg, &Origin::Sync { repo: db.name() }, extra, s.cols);
            }
        }
        print!("{out}");
        return 0;
    }
    let mut ret = 0;
    for target in &s.args.targets {
        match find_target(&s.sync, target) {
            Some((repo, pkg)) => dump_pkg_full(&mut out, pkg, &Origin::Sync { repo }, extra, s.cols),
            None => {
                print!("{out}");
                out.clear();
                msg::error(&format!("package '{target}' was not found"));
                ret = 1;
            }
        }
    }
    print!("{out}");
    ret
}

/// Resolves `name` or `repo/name` to the first matching package.
fn find_target<'a>(dbs: &'a [SyncDb], target: &str) -> Option<(&'a str, &'a PkgMeta)> {
    let (repo, name) = split_repo(target);
    dbs.iter()
        .filter(|db| repo.is_none_or(|r| r == db.name()))
        .find_map(|db| db.find(name).map(|p| (db.name(), p)))
}

fn list(s: &Session) -> i32 {
    let mut ret = 0;
    let mut selected: Vec<&SyncDb> = Vec::new();
    for target in &s.args.targets {
        match s.sync.iter().find(|db| db.name() == target) {
            Some(db) => selected.push(db),
            None => {
                msg::error(&format!("repository \"{target}\" was not found."));
                ret = 1;
            }
        }
    }
    if s.args.targets.is_empty() {
        selected = s.sync.iter().collect();
    }
    let c = colors();
    let mut out = String::new();
    for db in selected {
        for pkg in db.packages() {
            if s.args.quiet {
                let _ = writeln!(out, "{}", pkg.name);
                continue;
            }
            let _ = write!(
                out,
                "{}{} {}{} {}{}{}",
                c.repo,
                db.name(),
                c.title,
                pkg.name,
                c.version,
                pkg.version,
                c.nocolor
            );
            installed_marker(&mut out, s, pkg);
            out.push('\n');
        }
    }
    print!("{out}");
    ret
}

fn groups(s: &Session) -> i32 {
    let mut out = String::new();
    let all: Vec<(&str, &str)> = s
        .sync
        .iter()
        .flat_map(|db| db.packages())
        .flat_map(|p| p.groups.iter().map(move |g| (g.as_str(), p.name.as_str())))
        .collect();
    if s.args.targets.is_empty() {
        let mut seen: Vec<&str> = Vec::new();
        for (group, name) in &all {
            if s.args.groups > 1 {
                let _ = writeln!(out, "{group} {name}");
            } else if !seen.contains(group) {
                seen.push(group);
                let _ = writeln!(out, "{group}");
            }
        }
        print!("{out}");
        return i32::from(all.is_empty());
    }
    let mut ret = 0;
    for target in &s.args.targets {
        let members: Vec<_> = all.iter().filter(|(g, _)| g == target).collect();
        ret |= i32::from(members.is_empty());
        for (group, name) in members {
            if s.args.quiet {
                let _ = writeln!(out, "{name}");
            } else {
                let _ = writeln!(out, "{group} {name}");
            }
        }
    }
    print!("{out}");
    ret
}
