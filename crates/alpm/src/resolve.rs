// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_resolvedeps / resolvedep in lib/libalpm/deps.c.

//! Dependency resolution for installs: which sync packages to add, in install order.

use std::collections::HashSet;

use paku_formats::depend::Depend;
use paku_formats::meta::PkgMeta;

use crate::db::SyncDb;
use crate::db::git_provided::is_core_package;
use crate::deps::satisfies;
use crate::error::{Error, Result};

/// A package from a sync repository.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    /// Repository name.
    pub repo: &'a str,
    /// Package metadata.
    pub pkg: &'a PkgMeta,
}

impl<'a> Candidate<'a> {
    /// Whether this package satisfies `dep` directly or via `provides`.
    pub fn satisfies(&self, dep: &Depend) -> bool {
        satisfies(dep, &self.pkg.name, &self.pkg.version, &self.pkg.provides)
    }
}

/// Finds a package for `dep`: an exact name match in repository order first,
/// then the first package that provides it (port of `resolvedep`).
pub fn find_satisfier<'a>(dbs: &'a [SyncDb], dep: &Depend) -> Option<Candidate<'a>> {
    let by_name = dbs.iter().find_map(|db| {
        let pkg = db.find(&dep.name)?;
        let c = Candidate { repo: db.name(), pkg };
        c.satisfies(dep).then_some(c)
    });
    by_name.or_else(|| {
        dbs.iter().find_map(|db| {
            db.packages()
                .iter()
                .map(|pkg| Candidate { repo: db.name(), pkg })
                .find(|c| c.satisfies(dep))
        })
    })
}

/// Expands `targets` with their missing dependencies, dependencies first.
///
/// `installed(dep)` reports whether something already installed (paku's own
/// packages or Git's bundle) satisfies a dependency. Dependencies that only a core
/// package can satisfy (e.g. `sh` from `bash`) count as provided by Git.
pub fn resolve<'a>(
    dbs: &'a [SyncDb],
    targets: &[Candidate<'a>],
    installed: &dyn Fn(&Depend) -> bool,
) -> Result<Vec<Candidate<'a>>> {
    let mut plan: Vec<Candidate<'a>> = Vec::new();
    let mut visiting: HashSet<String> = HashSet::new();
    for &target in targets {
        visit(dbs, target, installed, &mut plan, &mut visiting)?;
    }
    Ok(plan)
}

fn visit<'a>(
    dbs: &'a [SyncDb],
    pkg: Candidate<'a>,
    installed: &dyn Fn(&Depend) -> bool,
    plan: &mut Vec<Candidate<'a>>,
    visiting: &mut HashSet<String>,
) -> Result<()> {
    if plan.iter().any(|p| p.pkg.name == pkg.pkg.name) || !visiting.insert(pkg.pkg.name.clone()) {
        return Ok(());
    }
    for dep_str in &pkg.pkg.depends {
        let dep = Depend::parse(dep_str);
        if installed(&dep) || plan.iter().any(|p| p.satisfies(&dep)) {
            continue;
        }
        let Some(sat) = find_satisfier(dbs, &dep) else {
            return Err(Error::UnsatisfiedDep {
                dep: dep_str.clone(),
                by: pkg.pkg.name.clone(),
            });
        };
        if is_core_package(&sat.pkg.name) {
            continue;
        }
        visit(dbs, sat, installed, plan, visiting)?;
    }
    plan.push(pkg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, version: &str, depends: &[&str], provides: &[&str]) -> PkgMeta {
        PkgMeta {
            name: name.into(),
            version: version.into(),
            depends: depends.iter().map(|s| s.to_string()).collect(),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            ..PkgMeta::default()
        }
    }

    fn db(name: &str, pkgs: Vec<PkgMeta>) -> SyncDb {
        let mut db = SyncDb::new(name, vec![]);
        db.set_packages(pkgs);
        db
    }

    fn names(plan: &[Candidate]) -> Vec<String> {
        plan.iter().map(|c| c.pkg.name.clone()).collect()
    }

    fn rsync_dbs() -> Vec<SyncDb> {
        vec![db(
            "msys",
            vec![
                pkg("rsync", "3.5.2-1", &["libiconv", "popt", "libxxhash", "sh"], &[]),
                pkg("popt", "1.19-1", &["libiconv", "libintl"], &[]),
                pkg("libxxhash", "0.8.4-1", &[], &[]),
                pkg("bash", "5.3-1", &[], &["sh"]),
            ],
        )]
    }

    #[test]
    fn dependencies_come_first_and_installed_are_skipped() {
        let dbs = rsync_dbs();
        let target = find_satisfier(&dbs, &Depend::parse("rsync")).unwrap();
        let installed = |d: &Depend| ["libiconv", "libintl"].contains(&d.name.as_str());
        let plan = resolve(&dbs, &[target], &installed).unwrap();
        assert_eq!(names(&plan), ["popt", "libxxhash", "rsync"]);
    }

    #[test]
    fn core_providers_are_never_planned() {
        let dbs = rsync_dbs();
        let target = find_satisfier(&dbs, &Depend::parse("rsync")).unwrap();
        let installed = |d: &Depend| ["libiconv", "libintl"].contains(&d.name.as_str());
        let plan = resolve(&dbs, &[target], &installed).unwrap();
        assert!(!names(&plan).contains(&"bash".to_string()));
    }

    #[test]
    fn missing_dependency_is_an_error() {
        let dbs = vec![db("r", vec![pkg("a", "1-1", &["nope>=2"], &[])])];
        let target = find_satisfier(&dbs, &Depend::parse("a")).unwrap();
        let err = resolve(&dbs, &[target], &|_| false).unwrap_err();
        assert!(matches!(err, Error::UnsatisfiedDep { ref dep, ref by } if dep == "nope>=2" && by == "a"));
    }

    #[test]
    fn cycles_terminate_and_providers_are_found() {
        let dbs = vec![db(
            "r",
            vec![
                pkg("a", "1-1", &["b", "virt"], &[]),
                pkg("b", "1-1", &["a"], &[]),
                pkg("c", "1-1", &[], &["virt=1"]),
            ],
        )];
        let target = find_satisfier(&dbs, &Depend::parse("a")).unwrap();
        let plan = resolve(&dbs, &[target], &|_| false).unwrap();
        assert_eq!(names(&plan), ["b", "c", "a"]);
    }

    #[test]
    fn exact_name_beats_provider() {
        let dbs = vec![db(
            "r",
            vec![pkg("prov", "1-1", &[], &["x"]), pkg("x", "1-1", &[], &[])],
        )];
        assert_eq!(find_satisfier(&dbs, &Depend::parse("x")).unwrap().pkg.name, "x");
    }
}
