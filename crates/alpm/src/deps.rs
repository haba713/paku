// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_depcmp and friends in lib/libalpm/deps.c.

//! Dependency satisfaction checks.

use std::cmp::Ordering;

use paku_formats::depend::{DepOp, Depend};

use crate::version::vercmp;

/// Whether `have` satisfies the constraint `op want`.
pub fn version_satisfies(op: DepOp, have: &str, want: &str) -> bool {
    let cmp = vercmp(have, want);
    match op {
        DepOp::Eq => cmp == Ordering::Equal,
        DepOp::Ge => cmp != Ordering::Less,
        DepOp::Le => cmp != Ordering::Greater,
        DepOp::Gt => cmp == Ordering::Greater,
        DepOp::Lt => cmp == Ordering::Less,
    }
}

/// Whether a package (`name`, `version`, `provides`) satisfies `dep`, either
/// directly or through a provision. An unversioned provision only satisfies
/// unversioned dependencies.
pub fn satisfies(dep: &Depend, name: &str, version: &str, provides: &[String]) -> bool {
    if dep.name == name && constraint_ok(dep, Some(version)) {
        return true;
    }
    provides.iter().map(|p| Depend::parse(p)).any(|prov| {
        let version = prov.constraint.as_ref().map(|(_, v)| v.as_str());
        prov.name == dep.name && constraint_ok(dep, version)
    })
}

fn constraint_ok(dep: &Depend, version: Option<&str>) -> bool {
    match (&dep.constraint, version) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some((op, want)), Some(have)) => version_satisfies(*op, have, want),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sat(dep: &str, name: &str, version: &str, provides: &[&str]) -> bool {
        let provides: Vec<String> = provides.iter().map(|s| s.to_string()).collect();
        satisfies(&Depend::parse(dep), name, version, &provides)
    }

    #[test]
    fn direct() {
        assert!(sat("foo", "foo", "1-1", &[]));
        assert!(sat("foo>=1.0", "foo", "1.2-1", &[]));
        assert!(sat("foo=1.2", "foo", "1.2-3", &[]));
        assert!(!sat("foo<1.0", "foo", "1.2-1", &[]));
        assert!(!sat("bar", "foo", "1-1", &[]));
    }

    #[test]
    fn provisions() {
        assert!(sat("sh", "bash", "5-1", &["sh"]));
        assert!(sat("cc>=10", "gcc", "15-1", &["cc=15"]));
        assert!(!sat("cc>=10", "gcc", "15-1", &["cc"]));
    }
}
