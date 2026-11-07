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
    /// A sync package declares that it replaces an installed one.
    Replaced { old: String, by: Candidate<'a> },
}

/// Compares every installed package with the first sync database that has it.
///
/// `skip` names packages already in the transaction; `ignored` applies
/// `IgnorePkg` / `IgnoreGroup`; `downgrade` is `-uu`.
pub fn sysupgrade<'a>(
    local: &[LocalPkg],
    dbs: &'a [SyncDb],
    skip: &[&str],
    downgrade: bool,
    ignored: &dyn Fn(&PkgMeta) -> bool,
) -> Vec<Outcome<'a>> {
    let mut out = Vec::new();
    for lpkg in local.iter().filter(|l| !skip.contains(&l.meta.name.as_str())) {
        if let Some(by) = replacer(lpkg, dbs, ignored) {
            out.push(Outcome::Replaced {
                old: lpkg.meta.name.clone(),
                by,
            });
        }
        let literal = dbs
            .iter()
            .find_map(|db| db.find(&lpkg.meta.name).map(|pkg| Candidate { repo: db.name(), pkg }));
        if let Some(c) = literal.and_then(|c| check_literal(lpkg, c, downgrade, ignored)) {
            out.push(c);
        }
    }
    out
}

fn check_literal<'a>(
    lpkg: &LocalPkg,
    c: Candidate<'a>,
    downgrade: bool,
    ignored: &dyn Fn(&PkgMeta) -> bool,
) -> Option<Outcome<'a>> {
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

/// The first non-ignored sync package whose `replaces` matches `lpkg`.
fn replacer<'a>(lpkg: &LocalPkg, dbs: &'a [SyncDb], ignored: &dyn Fn(&PkgMeta) -> bool) -> Option<Candidate<'a>> {
    dbs.iter()
        .flat_map(|db| db.packages().iter().map(move |pkg| Candidate { repo: db.name(), pkg }))
        .filter(|c| c.pkg.name != lpkg.meta.name && !ignored(c.pkg))
        .find(|c| {
            c.pkg
                .replaces
                .iter()
                .any(|r| satisfies(&Depend::parse(r), &lpkg.meta.name, &lpkg.meta.version, &[]))
        })
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

    #[test]
    fn newer_versions_are_upgraded_and_equal_ones_skipped() {
        let dbs = vec![db("r", vec![meta("a", "2-1"), meta("b", "1-1")])];
        let out = sysupgrade(
            &local(&[("a", "1-1"), ("b", "1-1"), ("foreign", "1-1")]),
            &dbs,
            &[],
            false,
            &|_| false,
        );
        assert_eq!(upgraded(&out), ["a-2-1"]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn first_repository_wins() {
        let dbs = vec![db("one", vec![meta("a", "1-1")]), db("two", vec![meta("a", "9-1")])];
        let out = sysupgrade(&local(&[("a", "1-1")]), &dbs, &[], false, &|_| false);
        assert!(out.is_empty());
    }

    #[test]
    fn older_repo_version_warns_or_downgrades() {
        let dbs = vec![db("r", vec![meta("a", "1-1")])];
        let lpkgs = local(&[("a", "2-1")]);
        let out = sysupgrade(&lpkgs, &dbs, &[], false, &|_| false);
        assert!(matches!(&out[..], [Outcome::LocalNewer { repo: "r", .. }]));
        let out = sysupgrade(&lpkgs, &dbs, &[], true, &|_| false);
        assert_eq!(upgraded(&out), ["a-1-1"]);
    }

    #[test]
    fn ignored_and_skipped_packages_are_not_upgraded() {
        let dbs = vec![db("r", vec![meta("a", "2-1"), meta("b", "2-1")])];
        let lpkgs = local(&[("a", "1-1"), ("b", "1-1")]);
        let out = sysupgrade(&lpkgs, &dbs, &["b"], false, &|p| p.name == "a");
        assert!(matches!(&out[..], [Outcome::Ignored { downgrade: false, .. }]));
    }

    #[test]
    fn replacements_are_reported() {
        let mut new = meta("new", "1-1");
        new.replaces = vec!["old<2".into()];
        let dbs = vec![db("r", vec![new, meta("old", "3-1")])];
        let out = sysupgrade(&local(&[("old", "1-1")]), &dbs, &[], false, &|_| false);
        assert!(matches!(&out[0], Outcome::Replaced { old, by } if old == "old" && by.pkg.name == "new"));
        assert_eq!(upgraded(&out), ["old-3-1"]);
    }
}
