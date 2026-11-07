// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _alpm_db_find_fileconflicts in lib/libalpm/conflict.c (MSYS2 variant:
// paths compare case-insensitively).

//! File conflicts: an incoming file that another package, or an untracked file on
//! disk, already occupies.

use std::collections::HashMap;

use paku_layout::Layout;

use crate::db::LocalDb;

/// Files a package in the transaction will install.
#[derive(Debug, Clone, Copy)]
pub struct PlannedFiles<'a> {
    /// Package name.
    pub name: &'a str,
    /// Prefix-relative paths; directories end with `/`.
    pub files: &'a [String],
}

/// A file that cannot be installed without `--overwrite`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Package that wants to install the file.
    pub package: String,
    /// Prefix-relative path.
    pub path: String,
    /// Package that already owns it; `None` for an untracked file on disk.
    pub owner: Option<String>,
}

/// Finds conflicts between the planned packages, installed packages and the disk.
/// Files of packages being replaced (same name) are not conflicts. `overwrite`
/// holds `--overwrite` globs, matched against `/<path>`.
pub fn find_conflicts(
    planned: &[PlannedFiles],
    local: &LocalDb,
    layout: &Layout,
    overwrite: &[String],
) -> Vec<Conflict> {
    let mut owners: HashMap<String, String> = HashMap::new();
    for pkg in local.packages() {
        if planned.iter().any(|p| p.name == pkg.meta.name) {
            continue;
        }
        for f in pkg.files.iter().filter(|f| !f.ends_with('/')) {
            owners.insert(f.to_ascii_lowercase(), pkg.meta.name.clone());
        }
    }
    let mut conflicts = Vec::new();
    let mut incoming: HashMap<String, &str> = HashMap::new();
    for pkg in planned {
        for path in pkg.files.iter().filter(|f| !f.ends_with('/')) {
            let key = path.to_ascii_lowercase();
            let owner = incoming
                .insert(key.clone(), pkg.name)
                .map(str::to_string)
                .or_else(|| owners.get(&key).cloned());
            let conflict = match owner {
                Some(owner) => Some(Some(owner)),
                None if is_untracked(layout, path, local) => Some(None),
                None => None,
            };
            if let Some(owner) = conflict.filter(|_| !overwrite_allows(overwrite, path)) {
                conflicts.push(Conflict {
                    package: pkg.name.to_string(),
                    path: path.clone(),
                    owner,
                });
            }
        }
    }
    conflicts
}

/// An existing file on disk that no installed package (including the one being
/// replaced) owns.
fn is_untracked(layout: &Layout, path: &str, local: &LocalDb) -> bool {
    let full = layout.install_path(path);
    if !full.exists() || full.is_dir() {
        return false;
    }
    !local
        .packages()
        .iter()
        .any(|p| p.files.iter().any(|f| f.eq_ignore_ascii_case(path)))
}

fn overwrite_allows(globs: &[String], path: &str) -> bool {
    let abs = format!("/{path}");
    globs
        .iter()
        .any(|g| glob_match(g.trim_start_matches('!'), &abs) && !g.starts_with('!'))
}

/// Minimal fnmatch: `*` matches any run (including `/`), `?` one character.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi].eq_ignore_ascii_case(&t[ti])) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::testutil::tempdir;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn untracked_file_on_disk_conflicts_unless_overwritten() {
        let dir = tempdir();
        let layout = Layout::new(dir.path(), "ucrt64");
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("bin").join("RG.EXE"), b"x").unwrap();
        let f = files(&["bin/", "bin/rg.exe"]);
        let planned = [PlannedFiles { name: "rg", files: &f }];
        let local = LocalDb::default();

        let c = find_conflicts(&planned, &local, &layout, &[]);
        assert_eq!(
            c,
            [Conflict {
                package: "rg".into(),
                path: "bin/rg.exe".into(),
                owner: None
            }]
        );
        assert!(find_conflicts(&planned, &local, &layout, &["/bin/*".into()]).is_empty());
    }

    #[test]
    fn two_planned_packages_with_same_file_conflict() {
        let dir = tempdir();
        let layout = Layout::new(dir.path(), "ucrt64");
        let (a, b) = (files(&["bin/x.exe"]), files(&["BIN/X.exe"]));
        let planned = [
            PlannedFiles { name: "a", files: &a },
            PlannedFiles { name: "b", files: &b },
        ];
        let c = find_conflicts(&planned, &LocalDb::default(), &layout, &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].owner.as_deref(), Some("a"));
    }

    #[test]
    fn globs() {
        assert!(glob_match("*", "/bin/x"));
        assert!(glob_match("/bin/*.exe", "/bin/rg.exe"));
        assert!(glob_match("/bin/r?.EXE", "/bin/rg.exe"));
        assert!(!glob_match("/lib/*", "/bin/rg.exe"));
    }
}
