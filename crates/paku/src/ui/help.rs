// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of usage() and version() in src/pacman/pacman.c.

//! `-h` and `-V` output.

use crate::args::Op;

const NAME: &str = "paku";

/// Help text for no operation (overview) or for one operation.
pub fn usage(op: Option<Op>) -> String {
    let Some(op) = op else {
        return overview();
    };
    let (header, mut opts) = op_options(op);
    opts.extend(trans_options(op));
    opts.extend(GLOBAL_OPTS.iter().map(|s| s.to_string()));
    opts.sort_by(|a, b| options_cmp(a, b));
    let mut out = format!("usage:  {NAME} {header}\noptions:\n");
    for o in opts {
        out.push_str(&o);
    }
    out
}

fn overview() -> String {
    let n = NAME;
    format!(
        "usage:  {n} <operation> [...]\noperations:\n    {n} {{-h --help}}\n    {n} {{-V --version}}\n    \
         {n} {{-D --database}} <options> <package(s)>\n    {n} {{-F --files}}    [options] [file(s)]\n    \
         {n} {{-Q --query}}    [options] [package(s)]\n    {n} {{-R --remove}}   [options] <package(s)>\n    \
         {n} {{-S --sync}}     [options] [package(s)]\n    {n} {{-T --deptest}}  [options] [package(s)]\n    \
         {n} {{-U --upgrade}}  [options] <file(s)>\n\nuse '{n} {{-h --help}}' with an operation for available options\n"
    )
}

/// Version and license text for `-V`.
pub fn version() -> String {
    let v = env!("CARGO_PKG_VERSION");
    format!(
        "Paku v{v} - pacman-compatible package manager\n\
         Copyright (C) 2026 paku contributors\n\
         \n\
         Based on pacman\n\
         Copyright (C) 2006-2024 Pacman Development Team\n\
         Copyright (C) 2002-2006 Judd Vinet\n\
         \n\
         Freely redistributable under the GNU General Public License.\n"
    )
}

fn op_options(op: Op) -> (&'static str, Vec<String>) {
    let (header, opts): (&str, &[&str]) = match op {
        Op::Remove => ("{-R --remove} [options] <package(s)>", REMOVE_OPTS),
        Op::Upgrade => ("{-U --upgrade} [options] <file(s)>", &[NEEDED]),
        Op::Query => ("{-Q --query} [options] [package(s)]", QUERY_OPTS),
        Op::Sync => ("{-S --sync} [options] [package(s)]", SYNC_OPTS),
        Op::Database => ("{-D --database} <options> <package(s)>", DATABASE_OPTS),
        Op::DepTest => ("{-T --deptest} [options] [package(s)]", &[]),
        Op::Files => ("{-F --files} [options] [file(s)]", FILES_OPTS),
    };
    (header, opts.iter().map(|s| s.to_string()).collect())
}

fn trans_options(op: Op) -> Vec<String> {
    let mut out: Vec<&str> = Vec::new();
    if matches!(op, Op::Sync | Op::Upgrade) {
        out.extend(UPGRADE_OPTS);
    }
    if matches!(op, Op::Sync | Op::Upgrade | Op::Remove) {
        out.extend(TRANS_OPTS);
    }
    out.into_iter().map(str::to_string).collect()
}

/// Sort order of help lines: short options before long-only, then by text.
fn options_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (a.trim_start(), b.trim_start());
    let a_long = a.starts_with("--");
    let b_long = b.starts_with("--");
    match (a_long, b_long) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        (true, true) => a[2..].cmp(&b[2..]),
        (false, false) => a.cmp(b),
    }
}

const NEEDED: &str = "      --needed         do not reinstall up to date packages\n";

const REMOVE_OPTS: &[&str] = &[
    "  -c, --cascade        remove packages and all packages that depend on them\n",
    "  -n, --nosave         remove configuration files\n",
    "  -s, --recursive      remove unnecessary dependencies\n                       (-ss includes explicitly installed dependencies)\n",
    "  -u, --unneeded       remove unneeded packages\n",
];

const QUERY_OPTS: &[&str] = &[
    "  -c, --changelog      view the changelog of a package\n",
    "  -d, --deps           list packages installed as dependencies [filter]\n",
    "  -e, --explicit       list packages explicitly installed [filter]\n",
    "  -g, --groups         view all members of a package group\n",
    "  -i, --info           view package information (-ii for backup files)\n",
    "  -k, --check          check that package files exist (-kk for file properties)\n",
    "  -l, --list           list the files owned by the queried package\n",
    "  -m, --foreign        list installed packages not found in sync db(s) [filter]\n",
    "  -n, --native         list installed packages only found in sync db(s) [filter]\n",
    "  -o, --owns <file>    query the package that owns <file>\n",
    "  -p, --file <package> query a package file instead of the database\n",
    "  -q, --quiet          show less information for query and search\n",
    "  -s, --search <regex> search locally-installed packages for matching strings\n",
    "  -t, --unrequired     list packages not (optionally) required by any\n                       package (-tt to ignore optdepends) [filter]\n",
    "  -u, --upgrades       list outdated packages [filter]\n",
];

const SYNC_OPTS: &[&str] = &[
    "  -c, --clean          remove old packages from cache directory (-cc for all)\n",
    "  -g, --groups         view all members of a package group\n                       (-gg to view all groups and members)\n",
    "  -i, --info           view package information (-ii for extended information)\n",
    "  -l, --list <repo>    view a list of packages in a repo\n",
    "  -q, --quiet          show less information for query and search\n",
    "  -s, --search <regex> search remote repositories for matching strings\n",
    "  -u, --sysupgrade     upgrade installed packages (-uu enables downgrades)\n",
    "  -y, --refresh        download fresh package databases from the server\n                       (-yy to force a refresh even if up to date)\n",
    NEEDED,
];

const DATABASE_OPTS: &[&str] = &[
    "      --asdeps         mark packages as non-explicitly installed\n",
    "      --asexplicit     mark packages as explicitly installed\n",
    "  -k, --check          test local database for validity (-kk for sync databases)\n",
    "  -q, --quiet          suppress output of success messages\n",
];

const FILES_OPTS: &[&str] = &[
    "  -l, --list           list the files owned by the queried package\n",
    "  -q, --quiet          show less information for query and search\n",
    "  -x, --regex          enable searching using regular expressions\n",
    "  -y, --refresh        download fresh package databases from the server\n                       (-yy to force a refresh even if up to date)\n",
    "      --machinereadable\n                       produce machine-readable output\n",
];

const UPGRADE_OPTS: &[&str] = &[
    "  -w, --downloadonly   download packages but do not install/upgrade anything\n",
    "      --overwrite <glob>\n                       overwrite conflicting files (can be used more than once)\n",
    "      --asdeps         install packages as non-explicitly installed\n",
    "      --asexplicit     install packages as explicitly installed\n",
    "      --ignore <pkg>   ignore a package upgrade (can be used more than once)\n",
    "      --ignoregroup <grp>\n                       ignore a group upgrade (can be used more than once)\n",
];

const TRANS_OPTS: &[&str] = &[
    "  -d, --nodeps         skip dependency version checks (-dd to skip all checks)\n",
    "      --assume-installed <package=version>\n                       add a virtual package to satisfy dependencies\n",
    "      --dbonly         only modify database entries, not package files\n",
    "      --noprogressbar  do not show a progress bar when downloading files\n",
    "      --noscriptlet    do not execute the install scriptlet if one exists\n",
    "  -p, --print          print the targets instead of performing the operation\n",
    "      --print-format <string>\n                       specify how the targets should be printed\n",
];

const GLOBAL_OPTS: &[&str] = &[
    "  -b, --dbpath <path>  set an alternate database location\n",
    "  -r, --root <path>    set an alternate installation root\n",
    "  -v, --verbose        be verbose\n",
    "      --arch <arch>    set an alternate architecture\n",
    "      --sysroot        operate on a mounted guest system (root-only)\n",
    "      --cachedir <dir> set an alternate package cache location\n",
    "      --hookdir <dir>  set an alternate hook location\n",
    "      --color <when>   colorize the output\n",
    "      --config <path>  set an alternate configuration file\n",
    "      --debug          display debug messages\n",
    "      --gpgdir <path>  set an alternate home directory for GnuPG\n",
    "      --logfile <path> set an alternate log file\n",
    "      --noconfirm      do not ask for any confirmation\n",
    "      --confirm        always ask for confirmation\n",
    "      --disable-download-timeout\n                       use relaxed timeouts for download\n",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_lists_operations() {
        let u = usage(None);
        assert!(u.starts_with("usage:  paku <operation> [...]\n"));
        assert!(u.contains("paku {-S --sync}     [options] [package(s)]"));
    }

    #[test]
    fn sync_help_is_sorted_short_first() {
        let u = usage(Some(Op::Sync));
        let b = u.find("  -b, --dbpath").unwrap();
        let y = u.find("  -y, --refresh").unwrap();
        let arch = u.find("      --arch").unwrap();
        assert!(b < y && y < arch);
        assert!(u.contains("--downloadonly") && u.contains("--nodeps"));
    }

    #[test]
    fn query_help_has_no_transaction_options() {
        assert!(!usage(Some(Op::Query)).contains("--nodeps"));
    }

    #[test]
    fn version_is_plain_text() {
        let v = version();
        assert!(v.starts_with(&format!("Paku v{} - ", env!("CARGO_PKG_VERSION"))));
        assert!(v.contains("Copyright (C) 2006-2024 Pacman Development Team"));
        assert!(v.lines().all(|l| !l.starts_with(' ') && l.len() <= 80));
    }
}
