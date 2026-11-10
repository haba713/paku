// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_remove_prepare, remove_prepare_cascade, remove_prepare_keep_needed and
// remove_notify_needed_optdepends in lib/libalpm/remove.c, plus the removal parts of
// alpm_checkdeps, _alpm_recursedeps, can_remove_package and _alpm_sortbydeps in
// lib/libalpm/deps.c.

//! Planning a removal: which installed packages go, in which order.

pub mod files;

use paku_formats::depend::Depend;
use paku_formats::meta::PkgMeta;

use crate::db::local::{LocalPkg, Reason};
use crate::deps::satisfies;

/// Removal flags (`-c`, `-s`/`-ss`, `-u`, `-d`/`-dd`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    /// `-c`: also remove packages that depend on the targets.
    pub cascade: bool,
    /// `-s`: also remove unneeded dependencies; `-ss` includes explicit ones.
    pub recursive: u8,
    /// `-u`: drop targets that are still needed instead of failing.
    pub unneeded: bool,
    /// `-d`: ignore versions in the dependency check; `-dd`: skip it.
    pub nodeps: u8,
}

/// A dependency of a remaining package that the removal would break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// The remaining package that needs it.
    pub target: String,
    /// The dependency string.
    pub depend: String,
    /// The package being removed that satisfied it.
    pub causing: String,
}

/// The result of [`prepare`].
#[derive(Debug, Default)]
pub struct Plan<'a> {
    /// Packages to remove, dependents before their dependencies.
    pub remove: Vec<&'a LocalPkg>,
    /// `(package, optdepend)` pairs of remaining packages that lose an optional dependency.
    pub optdepends: Vec<(String, String)>,
}

/// Expands and checks `targets` like `_alpm_remove_prepare`.
///
/// `provided(dep)` reports dependencies satisfied outside paku's database
/// (Git for Windows' bundle and core packages); those never break.
pub fn prepare<'a>(
    local: &'a [LocalPkg],
    targets: Vec<&'a LocalPkg>,
    flags: Flags,
    provided: &dyn Fn(&Depend) -> bool,
) -> Result<Plan<'a>, Vec<Missing>> {
    let mut remove = targets;
    if flags.recursive > 0 && !flags.cascade {
        recursedeps(local, &mut remove, flags.recursive > 1);
    }
    if flags.nodeps < 2 {
        let ctx = Check {
            local,
            provided,
            versions: flags.nodeps == 0,
        };
        let missing = ctx.checkdeps(&remove);
        if !missing.is_empty() {
            if flags.cascade {
                ctx.remove_prepare_cascade(&mut remove);
            } else if flags.unneeded {
                ctx.remove_prepare_keep_needed(&mut remove);
            } else {
                return Err(missing);
            }
        }
    }
    if flags.cascade && flags.recursive > 0 {
        recursedeps(local, &mut remove, flags.recursive > 1);
    }
    let optdepends = if flags.nodeps < 2 {
        needed_optdepends(local, &remove, provided)
    } else {
        Vec::new()
    };
    Ok(Plan {
        remove: sortbydeps(&remove),
        optdepends,
    })
}

struct Check<'a, 'p> {
    local: &'a [LocalPkg],
    provided: &'p dyn Fn(&Depend) -> bool,
    versions: bool,
}

impl<'a> Check<'a, '_> {
    /// Dependencies of remaining packages that only removed packages satisfied.
    fn checkdeps(&self, remove: &[&'a LocalPkg]) -> Vec<Missing> {
        let mut out = Vec::new();
        for lpkg in self.local.iter().filter(|l| !contains(remove, l)) {
            for dep_str in &lpkg.meta.depends {
                let dep = self.dep(dep_str);
                let Some(causing) = remove.iter().find(|r| sat(&dep, r)) else {
                    continue;
                };
                let kept = self.local.iter().any(|l| !contains(remove, l) && sat(&dep, l));
                if !kept && !(self.provided)(&dep) {
                    out.push(Missing {
                        target: lpkg.meta.name.clone(),
                        depend: dep_str.clone(),
                        causing: causing.meta.name.clone(),
                    });
                }
            }
        }
        out
    }

    /// `-c`: pulls every package with a broken dependency into the removal.
    fn remove_prepare_cascade(&self, remove: &mut Vec<&'a LocalPkg>) {
        loop {
            let missing = self.checkdeps(remove);
            if missing.is_empty() {
                return;
            }
            for m in missing {
                if let Some(p) = self.local.iter().find(|l| l.meta.name == m.target)
                    && !contains(remove, p)
                {
                    remove.push(p);
                }
            }
        }
    }

    /// `-u`: drops targets that remaining packages still need.
    fn remove_prepare_keep_needed(&self, remove: &mut Vec<&'a LocalPkg>) {
        loop {
            let missing = self.checkdeps(remove);
            if missing.is_empty() {
                return;
            }
            remove.retain(|r| !missing.iter().any(|m| m.causing == r.meta.name));
        }
    }

    fn dep(&self, s: &str) -> Depend {
        let mut d = Depend::parse(s);
        if !self.versions {
            d.constraint = None;
        }
        d
    }
}

/// `-s`: adds dependencies of the removal set that nothing else needs
/// (only those installed as dependencies unless `explicit`, i.e. `-ss`).
fn recursedeps<'a>(local: &'a [LocalPkg], remove: &mut Vec<&'a LocalPkg>, explicit: bool) {
    let mut i = 0;
    while i < remove.len() {
        for dep_str in &remove[i].meta.depends {
            let dep = Depend::parse(dep_str);
            let Some(deppkg) = local.iter().find(|l| sat(&dep, l)) else {
                continue;
            };
            if can_remove_package(local, deppkg, remove, explicit) {
                remove.push(deppkg);
            }
        }
        i += 1;
    }
}

fn can_remove_package(local: &[LocalPkg], pkg: &LocalPkg, remove: &[&LocalPkg], explicit: bool) -> bool {
    if contains(remove, pkg) || (!explicit && pkg.reason == Reason::Explicit) {
        return false;
    }
    !local.iter().any(|l| !contains(remove, l) && depends_on(l, pkg))
}

/// Optional dependencies of remaining packages that only removed packages satisfied.
fn needed_optdepends(
    local: &[LocalPkg],
    remove: &[&LocalPkg],
    provided: &dyn Fn(&Depend) -> bool,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for lpkg in local.iter().filter(|l| !contains(remove, l)) {
        for opt in &lpkg.meta.optdepends {
            let dep = Depend::parse(opt);
            let lost = remove.iter().any(|r| sat(&dep, r))
                && !local.iter().any(|l| !contains(remove, l) && sat(&dep, l))
                && !provided(&dep);
            if lost {
                out.push((lpkg.meta.name.clone(), opt.clone()));
            }
        }
    }
    out
}

/// Dependencies that break when `removed` packages are replaced by `added` ones
/// (the `alpm_checkdeps` call of `_alpm_sync_prepare`). Checks every package of
/// the resulting system: `added`, plus installed ones neither removed nor upgraded.
pub fn broken_by_replacement(
    local: &[LocalPkg],
    removed: &[&LocalPkg],
    added: &[&PkgMeta],
    provided: &dyn Fn(&Depend) -> bool,
) -> Vec<Missing> {
    let upgraded = |l: &LocalPkg| added.iter().any(|a| a.name == l.meta.name);
    let staying = local
        .iter()
        .filter(|l| !contains(removed, l) && !upgraded(l))
        .map(|l| &l.meta);
    let after: Vec<&PkgMeta> = added.iter().copied().chain(staying).collect();
    let mut out = Vec::new();
    for pkg in &after {
        for dep_str in &pkg.depends {
            let dep = Depend::parse(dep_str);
            let Some(causing) = removed.iter().find(|r| sat(&dep, r)) else {
                continue;
            };
            let kept = after.iter().any(|p| satisfies(&dep, &p.name, &p.version, &p.provides));
            if !kept && !provided(&dep) {
                out.push(Missing {
                    target: pkg.name.clone(),
                    depend: dep_str.clone(),
                    causing: causing.meta.name.clone(),
                });
            }
        }
    }
    out
}

/// Removal order: the reverse of install order, so dependents go first.
fn sortbydeps<'a>(pkgs: &[&'a LocalPkg]) -> Vec<&'a LocalPkg> {
    fn visit<'a>(p: &'a LocalPkg, pkgs: &[&'a LocalPkg], out: &mut Vec<&'a LocalPkg>, seen: &mut Vec<String>) {
        if seen.contains(&p.meta.name) {
            return;
        }
        seen.push(p.meta.name.clone());
        for dep in pkgs.iter().filter(|d| depends_on(p, d)) {
            visit(dep, pkgs, out, seen);
        }
        out.push(p);
    }
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for p in pkgs {
        visit(p, pkgs, &mut out, &mut seen);
    }
    out.reverse();
    out
}

fn depends_on(pkg: &LocalPkg, dep_pkg: &LocalPkg) -> bool {
    pkg.meta.depends.iter().any(|d| sat(&Depend::parse(d), dep_pkg))
}

fn sat(dep: &Depend, p: &LocalPkg) -> bool {
    satisfies(dep, &p.meta.name, &p.meta.version, &p.meta.provides)
}

fn contains(list: &[&LocalPkg], p: &LocalPkg) -> bool {
    list.iter().any(|x| x.meta.name == p.meta.name)
}

#[cfg(test)]
mod tests;
