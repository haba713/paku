// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use paku_formats::meta::PkgMeta;

fn pkg(name: &str, reason: Reason, depends: &[&str]) -> LocalPkg {
    LocalPkg {
        meta: PkgMeta {
            name: name.into(),
            version: "1-1".into(),
            depends: depends.iter().map(|s| s.to_string()).collect(),
            ..PkgMeta::default()
        },
        reason,
        install_date: 0,
        files: Vec::new(),
        backup: Vec::new(),
    }
}

/// app -> lib -> base; tool -> base; app and tool explicit.
fn local() -> Vec<LocalPkg> {
    vec![
        pkg("app", Reason::Explicit, &["lib"]),
        pkg("base", Reason::Depend, &[]),
        pkg("lib", Reason::Depend, &["base", "sh"]),
        pkg("tool", Reason::Explicit, &["base>=1"]),
    ]
}

fn targets<'a>(local: &'a [LocalPkg], names: &[&str]) -> Vec<&'a LocalPkg> {
    names
        .iter()
        .map(|n| local.iter().find(|l| l.meta.name == *n).unwrap())
        .collect()
}

fn names(plan: &Plan) -> Vec<String> {
    plan.remove.iter().map(|p| p.meta.name.clone()).collect()
}

fn run(names_: &[&str], flags: Flags) -> Result<Vec<String>, Vec<Missing>> {
    let l = local();
    let provided = |d: &Depend| d.name == "sh";
    prepare(&l, targets(&l, names_), flags, &provided).map(|p| names(&p))
}

#[test]
fn removing_a_leaf_is_fine() {
    assert_eq!(run(&["app"], Flags::default()).unwrap(), ["app"]);
}

#[test]
fn breaking_a_dependency_is_an_error() {
    let err = run(&["lib"], Flags::default()).unwrap_err();
    assert_eq!(
        err,
        [Missing {
            target: "app".into(),
            depend: "lib".into(),
            causing: "lib".into()
        }]
    );
}

#[test]
fn nodeps_once_ignores_versions_only() {
    assert!(
        run(
            &["base"],
            Flags {
                nodeps: 1,
                ..Flags::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        run(
            &["base"],
            Flags {
                nodeps: 2,
                ..Flags::default()
            }
        )
        .unwrap(),
        ["base"]
    );
}

#[test]
fn recursive_removes_unneeded_dependencies_in_order() {
    let flags = Flags {
        recursive: 1,
        ..Flags::default()
    };
    // base is still needed by tool
    assert_eq!(run(&["app"], flags).unwrap(), ["app", "lib"]);
    assert_eq!(run(&["app", "tool"], flags).unwrap(), ["tool", "app", "lib", "base"]);
}

#[test]
fn cascade_pulls_in_dependents() {
    let flags = Flags {
        cascade: true,
        ..Flags::default()
    };
    assert_eq!(run(&["base"], flags).unwrap(), ["app", "tool", "lib", "base"]);
}

#[test]
fn unneeded_drops_needed_targets() {
    let flags = Flags {
        unneeded: true,
        ..Flags::default()
    };
    assert_eq!(run(&["lib", "tool"], flags).unwrap(), ["tool"]);
}

#[test]
fn lost_optdepends_are_reported() {
    let mut l = local();
    l[3].meta.optdepends = vec!["lib: extra feature".into()];
    let plan = prepare(&l, targets(&l, &["app", "lib"]), Flags::default(), &|_| false).unwrap();
    assert_eq!(
        plan.optdepends,
        [("tool".to_string(), "lib: extra feature".to_string())]
    );
}

fn replacement_breaks(provides: &[&str]) -> Vec<Missing> {
    let l = local();
    let newlib = PkgMeta {
        name: "newlib".into(),
        version: "1-1".into(),
        provides: provides.iter().map(|s| s.to_string()).collect(),
        ..PkgMeta::default()
    };
    broken_by_replacement(&l, &targets(&l, &["lib"]), &[&newlib], &|_| false)
}

#[test]
fn replacement_that_provides_the_old_name_breaks_nothing() {
    assert!(replacement_breaks(&["lib"]).is_empty());
}

#[test]
fn replacement_without_provides_breaks_dependents() {
    assert_eq!(
        replacement_breaks(&[]),
        [Missing {
            target: "app".into(),
            depend: "lib".into(),
            causing: "lib".into(),
        }]
    );
}
