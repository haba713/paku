// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Role of remove_package_files / unlink_file / should_skip_file in
// lib/libalpm/remove.c, adapted to Windows.

//! Deleting an installed package's files from the install prefix.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use paku_layout::Layout;

use crate::backup::{self, BackupFile};
use crate::checksum::md5_file;
use crate::install::commit::backup_path;
use crate::install::conflict::matches_patterns;

/// How special files are treated while deleting a package's files.
#[derive(Debug, Clone, Copy)]
pub struct Options<'a> {
    /// Lowercase paths of directories that remaining packages own; never removed.
    pub keep_dirs: &'a HashSet<String>,
    /// The package's backup files; edited ones are saved as `.pacsave`.
    pub backup: &'a [BackupFile],
    /// `-n`: delete edited backup files too.
    pub nosave: bool,
    /// `NoUpgrade` patterns; matching files are left in place.
    pub no_upgrade: &'a [String],
}

/// Outcome of [`remove_files`].
#[derive(Debug, Default)]
pub struct Removed {
    /// Files in use that were renamed to `<file>.paku-old` but could not be deleted.
    pub leftovers: Vec<PathBuf>,
    /// Files that could not be removed at all, with the reason.
    pub failed: Vec<(PathBuf, String)>,
    /// Edited backup files and the `.pacsave` path each was renamed to.
    pub saved: Vec<(PathBuf, PathBuf)>,
}

/// Deletes `files` (relative to the prefix), deepest first. Directories are removed
/// only when empty and not in `keep_dirs`. Missing files are not an error.
pub fn remove_files(layout: &Layout, files: &[String], opts: &Options) -> Removed {
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort_by(|a, b| b.cmp(a));
    let mut out = Removed::default();
    for rel in sorted {
        let path = layout.install_path(rel);
        if rel.ends_with('/') {
            if !opts.keep_dirs.contains(&rel.to_ascii_lowercase()) {
                let _ = fs::remove_dir(&path);
            }
            continue;
        }
        if matches_patterns(opts.no_upgrade, rel) || save_modified(&path, rel, opts, &mut out) {
            continue;
        }
        unlink_file(path, &mut out);
    }
    out
}

/// Renames an edited backup file to `.pacsave`. Returns `true` when the file has
/// been dealt with and must not be deleted.
fn save_modified(path: &Path, rel: &str, opts: &Options, out: &mut Removed) -> bool {
    if opts.nosave {
        return false;
    }
    let Some(recorded) = backup::find(opts.backup, rel) else {
        return false;
    };
    // a file that cannot be read (e.g. already gone) counts as unmodified, like pacman
    let Ok(hash) = md5_file(path) else {
        return false;
    };
    if hash == recorded.hash {
        return false;
    }
    match backup::save(path) {
        Ok(saved) => out.saved.push((path.to_path_buf(), saved)),
        Err(e) => out.failed.push((path.to_path_buf(), e.to_string())),
    }
    true
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

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn options<'a>(keep_dirs: &'a HashSet<String>, backup: &'a [BackupFile]) -> Options<'a> {
        Options {
            keep_dirs,
            backup,
            nosave: false,
            no_upgrade: &[],
        }
    }

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
        let files = strings(&[
            "bin/",
            "bin/foo.exe",
            "share/",
            "share/foo/",
            "share/foo/data",
            "bin/gone.dll",
        ]);
        let keep: HashSet<String> = ["share/".to_string()].into();

        let r = remove_files(&layout, &files, &options(&keep, &[]));
        assert!(r.leftovers.is_empty() && r.failed.is_empty());
        assert!(!prefix.join("bin/foo.exe").exists());
        assert!(
            prefix.join("bin/other.exe").exists(),
            "non-empty dir keeps foreign files"
        );
        assert!(!prefix.join("share/foo").exists());
        assert!(prefix.join("share").is_dir(), "owned by a remaining package");
    }

    /// etc/edited.conf was changed after install, etc/same.conf was not.
    fn backup_fixture(prefix: &Path) -> (Vec<String>, Vec<BackupFile>) {
        fs::create_dir_all(prefix.join("etc")).unwrap();
        fs::write(prefix.join("etc/edited.conf"), b"mine").unwrap();
        fs::write(prefix.join("etc/same.conf"), b"abc").unwrap();
        let shipped = |path: &str| BackupFile {
            path: path.into(),
            hash: "900150983cd24fb0d6963f7d28e17f72".into(), // md5("abc")
        };
        (
            strings(&["etc/", "etc/edited.conf", "etc/same.conf"]),
            vec![shipped("etc/edited.conf"), shipped("etc/same.conf")],
        )
    }

    #[test]
    fn edited_backup_file_is_saved_as_pacsave() {
        let dir = tempdir();
        let layout = Layout::new(dir.path(), "ucrt64");
        let (files, backup) = backup_fixture(dir.path());
        let keep = HashSet::new();

        let r = remove_files(&layout, &files, &options(&keep, &backup));
        assert_eq!(
            r.saved,
            [(
                dir.path().join("etc").join("edited.conf"),
                dir.path().join("etc").join("edited.conf.pacsave")
            )]
        );
        assert_eq!(fs::read(dir.path().join("etc/edited.conf.pacsave")).unwrap(), b"mine");
        assert!(!dir.path().join("etc/edited.conf").exists());
        assert!(!dir.path().join("etc/same.conf").exists());
    }

    #[test]
    fn nosave_deletes_and_noupgrade_keeps() {
        let dir = tempdir();
        let layout = Layout::new(dir.path(), "ucrt64");
        let (files, backup) = backup_fixture(dir.path());
        let keep = HashSet::new();
        let no_upgrade = strings(&["etc/same.*"]);
        let opts = Options {
            nosave: true,
            no_upgrade: &no_upgrade,
            ..options(&keep, &backup)
        };

        let r = remove_files(&layout, &files, &opts);
        assert!(r.saved.is_empty());
        assert!(!dir.path().join("etc/edited.conf").exists());
        assert!(!dir.path().join("etc/edited.conf.pacsave").exists());
        assert!(dir.path().join("etc/same.conf").is_file());
    }
}
