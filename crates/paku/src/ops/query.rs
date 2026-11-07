// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/pacman/query.c.

//! `-Q`: queries paku's local database. Packages bundled with Git are not
//! listed here; they only satisfy dependencies.

use std::cmp::Ordering;
use std::fmt::Write;
use std::path::Path;

use alpm::db::local::{LocalPkg, Reason};
use alpm::search::Search;
use alpm::vercmp;

use crate::session::Session;
use crate::ui::color::colors;
use crate::ui::info::{Origin, dump_pkg_full};
use crate::ui::msg;

/// Runs `-Q`.
pub fn run(s: &mut Session) -> i32 {
    let a = &s.args;
    if a.groups > 0 || a.check > 0 || a.changelog || a.isfile {
        msg::error("this query option is not implemented yet");
        return 1;
    }
    if a.search {
        return search(s);
    }
    if a.owns {
        return owns(s);
    }
    if (a.upgrades || a.foreign || a.native) && !s.load_sync_dbs() {
        return 1;
    }
    let (pkgs, ret) = select(s);
    let mut out = String::new();
    for pkg in pkgs.iter().filter(|p| passes_filters(s, p)) {
        display(&mut out, s, pkg);
    }
    print!("{out}");
    ret
}

/// Target packages, or all installed packages without targets.
fn select(s: &Session) -> (Vec<&LocalPkg>, i32) {
    if s.args.targets.is_empty() {
        return (s.local.packages().iter().collect(), 0);
    }
    let mut ret = 0;
    let mut pkgs = Vec::new();
    for target in &s.args.targets {
        match s.local.find(target) {
            Some(p) => pkgs.push(p),
            None => {
                msg::error(&format!("package '{target}' was not found"));
                ret = 1;
            }
        }
    }
    (pkgs, ret)
}

/// `-d`, `-e`, `-m`, `-n`, `-u` filters (`-t` is not yet supported).
fn passes_filters(s: &Session, pkg: &LocalPkg) -> bool {
    let a = &s.args;
    let in_sync = s.sync.iter().find_map(|db| db.find(&pkg.meta.name));
    let ok = !(a.deps && pkg.reason != Reason::Depend)
        && !(a.explicit && pkg.reason != Reason::Explicit)
        && !(a.foreign && in_sync.is_some())
        && !(a.native && in_sync.is_none());
    if !ok || !a.upgrades {
        return ok;
    }
    in_sync.is_some_and(|sp| vercmp(&sp.version, &pkg.meta.version) == Ordering::Greater)
}

fn display(out: &mut String, s: &Session, pkg: &LocalPkg) {
    let a = &s.args;
    if a.info > 0 {
        let origin = Origin::Local {
            reason: pkg.reason,
            install_date: pkg.install_date,
            has_scriptlet: false,
        };
        dump_pkg_full(out, &pkg.meta, &origin, a.info > 1, s.cols);
    }
    if a.list {
        list_files(out, s, pkg);
    }
    if a.info > 0 || a.list {
        return;
    }
    if a.quiet {
        let _ = writeln!(out, "{}", pkg.meta.name);
        return;
    }
    let c = colors();
    let _ = write!(
        out,
        "{}{}{} {}{}{}",
        c.title, pkg.meta.name, c.nocolor, c.version, pkg.meta.version, c.nocolor
    );
    if a.upgrades {
        let newer = s.sync.iter().find_map(|db| db.find(&pkg.meta.name));
        if let Some(n) = newer {
            let _ = write!(out, " -> {}{}{}", c.version, n.version, c.nocolor);
        }
    }
    out.push('\n');
}

/// `-Ql`: `name <prefix>/<file>`, forward slashes.
fn list_files(out: &mut String, s: &Session, pkg: &LocalPkg) {
    let prefix = slashes(&s.conf.prefix);
    for file in &pkg.files {
        if s.args.quiet {
            let _ = writeln!(out, "{prefix}/{file}");
        } else {
            let _ = writeln!(out, "{} {prefix}/{file}", pkg.meta.name);
        }
    }
}

fn search(s: &Session) -> i32 {
    let searcher = match Search::new(&s.args.targets) {
        Ok(x) => x,
        Err(e) => {
            msg::error(&e.to_string());
            return 1;
        }
    };
    let c = colors();
    let mut out = String::new();
    for pkg in s.local.packages().iter().filter(|p| searcher.matches(&p.meta)) {
        if s.args.quiet {
            let _ = writeln!(out, "{}", pkg.meta.name);
            continue;
        }
        let _ = writeln!(
            out,
            "{}local/{}{} {}{}{}\n    {}",
            c.repo, c.title, pkg.meta.name, c.version, pkg.meta.version, c.nocolor, pkg.meta.desc
        );
    }
    print!("{out}");
    i32::from(out.is_empty())
}

/// `-Qo`: which package owns each target file (case-insensitive, as on MSYS2).
fn owns(s: &Session) -> i32 {
    let prefix = slashes(&s.conf.prefix).to_ascii_lowercase();
    let mut ret = 0;
    for target in &s.args.targets {
        let abs = std::path::absolute(target).unwrap_or_else(|_| target.into());
        let abs = slashes(&abs).to_ascii_lowercase();
        let rel = abs.strip_prefix(&prefix).map(|r| r.trim_start_matches('/'));
        let owner = rel.and_then(|rel| {
            s.local
                .packages()
                .iter()
                .find(|p| p.files.iter().any(|f| f.eq_ignore_ascii_case(rel)))
        });
        match owner {
            Some(p) if s.args.quiet => println!("{}", p.meta.name),
            Some(p) => println!("{target} is owned by {} {}", p.meta.name, p.meta.version),
            None => {
                msg::error(&format!("No package owns {target}"));
                ret = 1;
            }
        }
    }
    ret
}

fn slashes(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}
