// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn p(s: &str) -> Result<Args, ArgError> {
    let args: Vec<String> = s.split_whitespace().map(String::from).collect();
    parse(&args)
}

fn invalid(s: &str) -> String {
    match p(s) {
        Err(ArgError::Invalid(m)) => m,
        other => panic!("expected Invalid for {s}, got {other:?}"),
    }
}

#[test]
fn sync_upgrade_with_counters() {
    let a = p("-Syyuu --noconfirm foo").unwrap();
    assert_eq!(a.op, Some(Op::Sync));
    assert_eq!((a.refresh, a.sysupgrade), (2, 2));
    assert_eq!(a.noconfirm, Some(true));
    assert_eq!(a.targets, ["foo"]);
}

#[test]
fn letters_depend_on_operation() {
    assert!(p("-Ss x").unwrap().search);
    assert_eq!(p("-Rs x").unwrap().recursive, 1);
    assert_eq!(p("-Rss x").unwrap().recursive, 2);
    assert!(p("-Qs x").unwrap().search);
    assert!(p("-Qo x").unwrap().owns);
    assert!(p("-Rc x").unwrap().cascade);
    assert_eq!(p("-Sc").unwrap().clean, 1);
    assert!(p("-Qc x").unwrap().changelog);
}

#[test]
fn query_flags() {
    let a = p("-Qqe").unwrap();
    assert!(a.quiet && a.explicit);
    assert_eq!(p("-Qtt").unwrap().unrequired, 2);
    assert_eq!(p("-Qii x").unwrap().info, 2);
}

#[test]
fn trans_flags_and_lists() {
    let a = p("-S --ignore a,b --ignore c --overwrite '*' -dd --dbonly x").unwrap();
    assert_eq!(a.ignore, ["a", "b", "c"]);
    assert_eq!(a.nodeps, 2);
    assert!(a.dbonly && a.noscriptlet);
}

#[test]
fn global_options() {
    let a = p("-Q -b /db --root=/r -v -v --color never --debug").unwrap();
    assert_eq!(a.dbpath.as_deref(), Some("/db"));
    assert_eq!(a.root.as_deref(), Some("/r"));
    assert_eq!(a.verbose, 2);
    assert_eq!(a.color, Some(ColorMode::Never));
    assert_eq!(a.debug, 1);
    assert!(a.noprogressbar);
}

#[test]
fn operation_errors() {
    assert_eq!(invalid("-SQ"), "only one operation may be used at a time");
    assert_eq!(invalid("-SS"), "only one operation may be used at a time");
    assert_eq!(p("-Z"), Err(ArgError::Getopt("invalid option -- 'Z'".into())));
}

#[test]
fn letter_not_valid_for_operation() {
    assert_eq!(invalid("-Qy"), "invalid option '-y'");
    assert_eq!(invalid("-R --needed x"), "invalid option '--needed'");
    assert_eq!(invalid("-T -s"), "invalid option '-s'");
}

#[test]
fn bad_values() {
    assert_eq!(
        invalid("-S --color sometimes"),
        "invalid argument 'sometimes' for --color"
    );
    assert_eq!(invalid("-S --debug=3"), "'3' is not a valid debug level");
    assert_eq!(invalid("-S --ask x"), "'x' is not a valid ask level");
}

#[test]
fn conflicting_combinations() {
    assert_eq!(
        invalid("-Ssi x"),
        "invalid option: '--info' and '--search' may not be used together"
    );
    assert_eq!(
        invalid("-Qde"),
        "invalid option: '--deps' and '--explicit' may not be used together"
    );
    assert_eq!(
        invalid("-Scy"),
        "invalid option: '--clean' and '--refresh' may not be used together"
    );
    assert_eq!(
        invalid("-Rn -p x"),
        "invalid option: '--nosave' and '--print' may not be used together"
    );
}

#[test]
fn help_and_version_short_circuit() {
    let a = p("-Sh --bogus-is-ok-later").err();
    assert!(matches!(a, Some(ArgError::Getopt(_))), "getopt errors still come first");
    let a = p("-Sh").unwrap();
    assert!(a.help);
    assert_eq!(a.op, Some(Op::Sync));
    assert!(p("-V").unwrap().version);
    assert_eq!(p("").unwrap().op, None);
}
