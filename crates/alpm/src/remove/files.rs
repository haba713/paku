// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Role of remove_package_files / unlink_file in lib/libalpm/remove.c, adapted to Windows.

//! Deleting an installed package's files from the install prefix.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use paku_layout::Layout;

use crate::install::commit::backup_path;

/// Outcome of [`remove_files`].
#[derive(Debug, Default)]
pub struct Removed {
    /// Files in use that were renamed to `<file>.paku-old` but could not be deleted.
    pub leftovers: Vec<PathBuf>,
    /// Files that could not be removed at all, with the reason.
    pub failed: Vec<(PathBuf, String)>,
}

/// Deletes `files` (relative to the prefix), deepest first. Directories are removed
/// only when empty and not listed in `keep_dirs` (lowercase paths of directories
/// that remaining packages own). Missing files are not an error.
pub fn remove_files(layout: &Layout, files: &[String], keep_dirs: &HashSet<String>) -> Removed {
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort_by(|a, b| b.cmp(a));
    let mut out = Removed::default();
    for rel in sorted {
        let path = layout.install_path(rel);
        if rel.ends_with('/') {
            if !keep_dirs.contains(&rel.to_ascii_lowercase()) {
                let _ = fs::remove_dir(&path);
            }
            continue;
        }
        unlink_file(path, &mut out);
    }
    out
}

/// A running `.exe`/`.dll` can't be deleted but can be renamed out of the way.
fn unlink_file(path: PathBuf, out: &mut Removed) {
    let err = match fs::remove_file(&path) {
        Ok(()) => return,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => e,
    };
    let backup = backup_path(&path);
    if fs::rename(&path, &backup).is_err() {
        out.failed.push((path, err.to_string()));
        return;
    }
    if fs::remove_file(&backup).is_err() {
        out.leftovers.push(backup);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::testutil::tempdir;

    #[test]
    fn removes_files_and_empty_unowned_dirs() {
        let dir = tempdir();
        let prefix = dir.path();
        let layout = Layout::new(prefix, "ucrt64");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::create_dir_all(prefix.join("share/foo")).unwrap();
        fs::write(prefix.join("bin/foo.exe"), b"x").unwrap();
        fs::write(prefix.join("bin/other.exe"), b"x").unwrap();
        fs::write(prefix.join("share/foo/data"), b"x").unwrap();
        let files: Vec<String> = [
            "bin/",
            "bin/foo.exe",
            "share/",
            "share/foo/",
            "share/foo/data",
            "bin/gone.dll",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let keep: HashSet<String> = ["share/".to_string()].into();

        let r = remove_files(&layout, &files, &keep);
        assert!(r.leftovers.is_empty() && r.failed.is_empty());
        assert!(!prefix.join("bin/foo.exe").exists());
        assert!(
            prefix.join("bin/other.exe").exists(),
            "non-empty dir keeps foreign files"
        );
        assert!(!prefix.join("share/foo").exists());
        assert!(prefix.join("share").is_dir(), "owned by a remaining package");
    }
}
