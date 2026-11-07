// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Role of commit_single_pkg in lib/libalpm/add.c, adapted to Windows.

//! Moving a staged package into the install prefix.
//!
//! Existing files are first renamed to `<file>.paku-old` (a running `.exe` or
//! `.dll` can be renamed but not overwritten). If anything fails, moved files are
//! removed and backups restored, so the prefix is left as it was.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use paku_layout::Layout;

use crate::error::{Result, io_err};

/// Suffix of files replaced during an install.
pub const OLD_SUFFIX: &str = ".paku-old";

/// Outcome of a successful commit.
#[derive(Debug, Default)]
pub struct Committed {
    /// Backups that could not be deleted (files in use); removed on a later run.
    pub leftovers: Vec<PathBuf>,
}

#[derive(Default)]
struct Journal {
    moved: Vec<PathBuf>,
    backups: Vec<(PathBuf, PathBuf)>,
}

/// Installs `files` from `staging` into the prefix, then deletes files of the
/// previous version (`old_files`) that the new version no longer has.
pub fn commit(staging: &Path, layout: &Layout, files: &[String], old_files: &[String]) -> Result<Committed> {
    let mut journal = Journal::default();
    if let Err(e) = move_all(staging, layout, files, &mut journal) {
        rollback(&journal);
        return Err(e);
    }
    let mut done = Committed::default();
    for (_, backup) in &journal.backups {
        if fs::remove_file(backup).is_err() {
            done.leftovers.push(backup.clone());
        }
    }
    remove_stale(layout, files, old_files);
    Ok(done)
}

fn move_all(staging: &Path, layout: &Layout, files: &[String], journal: &mut Journal) -> Result<()> {
    for rel in files {
        let dest = layout.install_path(rel);
        if rel.ends_with('/') {
            fs::create_dir_all(&dest).map_err(io_err(&dest))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        if dest.exists() {
            let backup = backup_path(&dest);
            fs::rename(&dest, &backup).map_err(io_err(&dest))?;
            journal.backups.push((dest.clone(), backup));
        }
        let src = Layout::new(staging, layout.env()).install_path(rel);
        fs::rename(&src, &dest).map_err(io_err(&dest))?;
        journal.moved.push(dest);
    }
    Ok(())
}

fn rollback(journal: &Journal) {
    for dest in journal.moved.iter().rev() {
        let _ = fs::remove_file(dest);
    }
    for (dest, backup) in journal.backups.iter().rev() {
        let _ = fs::rename(backup, dest);
    }
}

/// `<file>.paku-old`, or `<file>.paku-old.N` if a leftover blocks it.
fn backup_path(dest: &Path) -> PathBuf {
    let base = format!("{}{OLD_SUFFIX}", dest.display());
    let first = PathBuf::from(&base);
    if !first.exists() || fs::remove_file(&first).is_ok() {
        return first;
    }
    (1..)
        .map(|n| PathBuf::from(format!("{base}.{n}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Deletes old files missing from the new list, then their directories if empty.
fn remove_stale(layout: &Layout, files: &[String], old_files: &[String]) {
    let keep: HashSet<String> = files.iter().map(|f| f.to_ascii_lowercase()).collect();
    let mut stale: Vec<&String> = old_files
        .iter()
        .filter(|f| !keep.contains(&f.to_ascii_lowercase()))
        .collect();
    stale.sort_by(|a, b| b.cmp(a));
    for rel in stale {
        let path = layout.install_path(rel);
        let _ = if rel.ends_with('/') {
            fs::remove_dir(&path)
        } else {
            fs::remove_file(&path)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::testutil::tempdir;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn stage(staging: &Path, rel: &str, data: &[u8]) {
        let p = staging.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    #[test]
    fn installs_replaces_and_removes_stale_files() {
        let dir = tempdir();
        let (staging, prefix) = (dir.path().join("s"), dir.path().join("p"));
        let layout = Layout::new(&prefix, "ucrt64");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::write(prefix.join("bin/a.exe"), b"old").unwrap();
        fs::write(prefix.join("bin/gone.dll"), b"old").unwrap();
        stage(&staging, "bin/a.exe", b"new");

        let new = files(&["bin/", "bin/a.exe"]);
        let done = commit(&staging, &layout, &new, &files(&["bin/", "bin/a.exe", "bin/gone.dll"])).unwrap();
        assert!(done.leftovers.is_empty());
        assert_eq!(fs::read(prefix.join("bin/a.exe")).unwrap(), b"new");
        assert!(!prefix.join("bin/gone.dll").exists());
        assert!(!prefix.join("bin/a.exe.paku-old").exists());
    }

    #[test]
    fn failure_rolls_back() {
        let dir = tempdir();
        let (staging, prefix) = (dir.path().join("s"), dir.path().join("p"));
        let layout = Layout::new(&prefix, "ucrt64");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::write(prefix.join("bin/a.exe"), b"old").unwrap();
        stage(&staging, "bin/a.exe", b"new");
        // b.exe is listed but not staged, so its move fails after a.exe was replaced
        let err = commit(&staging, &layout, &files(&["bin/a.exe", "bin/b.exe"]), &[]);
        assert!(err.is_err());
        assert_eq!(fs::read(prefix.join("bin/a.exe")).unwrap(), b"old");
        assert!(!prefix.join("bin/a.exe.paku-old").exists());
    }
}
