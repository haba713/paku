// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of alpm_sync_sysupgrade / check_literal / check_replacers in lib/libalpm/sync.c.

//! `-Su`: which installed packages have a different version in the sync databases.

use paku_formats::depend::Depend;
use paku_formats::meta::PkgMeta;

use crate::db::SyncDb;
use crate::db::local::LocalPkg;
use crate::deps::satisfies;
use crate::resolve::Candidate;
use crate::version::vercmp;

/// What a full upgrade decided for one installed package.
#[derive(Debug, Clone)]
pub enum Outcome<'a> {
    /// A newer version is available (or an older one with `-uu`).
    Upgrade(Candidate<'a>),
    /// An upgrade or downgrade skipped because of `IgnorePkg` / `IgnoreGroup`.
    Ignored {
        name: String,
        local: String,
        sync: String,
        downgrade: bool,
    },
    /// The installed version is newer than the repository's (without `-uu`).
    LocalNewer {
        name: String,
        local: String,
        repo: &'a str,
        sync: String,
    },
    /// A sync package replaces an installed one: install `by`, remove `old`.
    Replaced { old: &'a LocalPkg, by: Candidate<'a> },
    /// A replacement skipped because of `IgnorePkg` / `IgnoreGroup`.
    IgnoredReplacement { old: &'a LocalPkg, by: Candidate<'a> },
}

/// Choices the front-end makes during a full upgrade.
#[derive(Clone, Copy)]
pub struct Policy<'p> {
    /// `-uu`: also move to older repository versions.
    pub downgrade: bool,
    /// `IgnorePkg` / `IgnoreGroup`.
    pub ignored: &'p dyn Fn(&PkgMeta) -> bool,
    /// Asked before an installed package is replaced by a sync package.
    pub replace: &'p dyn Fn(&LocalPkg, &Candidate) -> bool,
}

/// Checks every installed package against the first sync database that has a
/// replacement for it or, failing that, a package of the same name.
///
/// `skip` names packages already in the transaction.
pub fn sysupgrade<'a>(local: &'a [LocalPkg], dbs: &'a [SyncDb], skip: &[&str], policy: &Policy) -> Vec<Outcome<'a>> {
    let mut out = Vec::new();
    for lpkg in local.iter().filter(|l| !skip.contains(&l.meta.name.as_str())) {
        for db in dbs {
            if check_replacers(lpkg, db, policy, &mut out) {
                break;
            }
            if let Some(pkg) = db.find(&lpkg.meta.name) {
                out.extend(check_literal(lpkg, Candidate { repo: db.name(), pkg }, policy));
                break;
            }
        }
    }
    out
}

/// Adds the outcome for every package in `db` that replaces `lpkg`. Returns
/// whether a replacement was accepted, which ends the search for `lpkg`.
fn check_replacers<'a>(lpkg: &'a LocalPkg, db: &'a SyncDb, policy: &Policy, out: &mut Vec<Outcome<'a>>) -> bool {
    let mut accepted = false;
    let replacers = db
        .packages()
        .iter()
        .filter(|p| p.name != lpkg.meta.name && replaces(p, lpkg));
    for pkg in replacers {
        let by = Candidate { repo: db.name(), pkg };
        if (policy.ignored)(pkg) || (policy.ignored)(&lpkg.meta) {
            out.push(Outcome::IgnoredReplacement { old: lpkg, by });
        } else if (policy.replace)(lpkg, &by) {
            out.push(Outcome::Replaced { old: lpkg, by });
            accepted = true;
        }
    }
    accepted
}

/// Whether one of `pkg`'s `replaces` entries names `lpkg` (provides don't count).
fn replaces(pkg: &PkgMeta, lpkg: &LocalPkg) -> bool {
    pkg.replaces
        .iter()
        .any(|r| satisfies(&Depend::parse(r), &lpkg.meta.name, &lpkg.meta.version, &[]))
}

fn check_literal<'a>(lpkg: &LocalPkg, c: Candidate<'a>, policy: &Policy) -> Option<Outcome<'a>> {
    let (downgrade, ignored) = (policy.downgrade, policy.ignored);
    let cmp = vercmp(&c.pkg.version, &lpkg.meta.version);
    if cmp.is_eq() || (cmp.is_lt() && !downgrade) {
        return cmp.is_lt().then(|| Outcome::LocalNewer {
            name: lpkg.meta.name.clone(),
            local: lpkg.meta.version.clone(),
            repo: c.repo,
            sync: c.pkg.version.clone(),
        });
    }
    if ignored(c.pkg) || ignored(&lpkg.meta) {
        return Some(Outcome::Ignored {
            name: lpkg.meta.name.clone(),
            local: lpkg.meta.version.clone(),
            sync: c.pkg.version.clone(),
            downgrade: cmp.is_lt(),
        });
    }
    Some(Outcome::Upgrade(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::local::Reason;

    fn meta(name: &str, version: &str) -> PkgMeta {
        PkgMeta {
            name: name.into(),
            version: version.into(),
            ..PkgMeta::default()
        }
    }

    fn local(pkgs: &[(&str, &str)]) -> Vec<LocalPkg> {
        pkgs.iter()
            .map(|(n, v)| LocalPkg {
                meta: meta(n, v),
                reason: Reason::Explicit,
                install_date: 0,
                files: Vec::new(),
                backup: Vec::new(),
            })
            .collect()
    }

    fn db(name: &str, pkgs: Vec<PkgMeta>) -> SyncDb {
        let mut db = SyncDb::new(name, vec![]);
        db.set_packages(pkgs);
        db
    }

    fn upgraded(out: &[Outcome]) -> Vec<String> {
        out.iter()
            .filter_map(|o| match o {
                Outcome::Upgrade(c) => Some(format!("{}-{}", c.pkg.name, c.pkg.version)),
                _ => None,
            })
            .collect()
    }

    /// Plain upgrade policy; `ignore` names the one package in `IgnorePkg`.
    fn run<'a>(
        lpkgs: &'a [LocalPkg],
        dbs: &'a [SyncDb],
        skip: &[&str],
        downgrade: bool,
        ignore: &str,
    ) -> Vec<Outcome<'a>> {
        let policy = Policy {
            downgrade,
            ignored: &|p| p.name == ignore,
            replace: &|_, _| true,
        };
        sysupgrade(lpkgs, dbs, skip, &policy)
    }

    #[test]
    fn newer_versions_are_upgraded_and_equal_ones_skipped() {
        let dbs = vec![db("r", vec![meta("a", "2-1"), meta("b", "1-1")])];
        let lpkgs = local(&[("a", "1-1"), ("b", "1-1"), ("foreign", "1-1")]);
        let out = run(&lpkgs, &dbs, &[], false, "");
        assert_eq!(upgraded(&out), ["a-2-1"]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn first_repository_wins() {
        let dbs = vec![db("one", vec![meta("a", "1-1")]), db("two", vec![meta("a", "9-1")])];
        let lpkgs = local(&[("a", "1-1")]);
        assert!(run(&lpkgs, &dbs, &[], false, "").is_empty());
    }

    #[test]
    fn older_repo_version_warns_or_downgrades() {
        let dbs = vec![db("r", vec![meta("a", "1-1")])];
        let lpkgs = local(&[("a", "2-1")]);
        let out = run(&lpkgs, &dbs, &[], false, "");
        assert!(matches!(&out[..], [Outcome::LocalNewer { repo: "r", .. }]));
        let out = run(&lpkgs, &dbs, &[], true, "");
        assert_eq!(upgraded(&out), ["a-1-1"]);
    }

    #[test]
    fn ignored_and_skipped_packages_are_not_upgraded() {
        let dbs = vec![db("r", vec![meta("a", "2-1"), meta("b", "2-1")])];
        let lpkgs = local(&[("a", "1-1"), ("b", "1-1")]);
        let out = run(&lpkgs, &dbs, &["b"], false, "a");
        assert!(matches!(&out[..], [Outcome::Ignored { downgrade: false, .. }]));
    }

    /// `new` replaces `old<2`; the repository still carries a newer `old`.
    fn replacing_dbs() -> Vec<SyncDb> {
        let mut new = meta("new", "1-1");
        new.replaces = vec!["old<2".into()];
        vec![db("r", vec![new, meta("old", "3-1")])]
    }

    #[test]
    fn replacement_wins_over_upgrade() {
        let dbs = replacing_dbs();
        let lpkgs = local(&[("old", "1-1")]);
        let out = run(&lpkgs, &dbs, &[], false, "");
        assert!(matches!(
            &out[..],
            [Outcome::Replaced { old, by }] if old.meta.name == "old" && by.pkg.name == "new"
        ));
    }

    #[test]
    fn replaces_respects_version_constraint() {
        let dbs = replacing_dbs();
        let lpkgs = local(&[("old", "2-1")]);
        assert_eq!(upgraded(&run(&lpkgs, &dbs, &[], false, "")), ["old-3-1"]);
    }

    #[test]
    fn declined_or_ignored_replacement_falls_back_to_upgrade() {
        let dbs = replacing_dbs();
        let lpkgs = local(&[("old", "1-1")]);
        let declined = Policy {
            downgrade: false,
            ignored: &|_| false,
            replace: &|_, _| false,
        };
        assert_eq!(upgraded(&sysupgrade(&lpkgs, &dbs, &[], &declined)), ["old-3-1"]);

        let out = run(&lpkgs, &dbs, &[], false, "new");
        assert!(matches!(&out[0], Outcome::IgnoredReplacement { by, .. } if by.pkg.name == "new"));
        assert_eq!(upgraded(&out), ["old-3-1"]);
    }
}
