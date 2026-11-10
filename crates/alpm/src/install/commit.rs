// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Role of commit_single_pkg / extract_single_file in lib/libalpm/add.c, adapted to Windows.

//! Moving a staged package into the install prefix.
//!
//! Existing files are first renamed to `<file>.paku-old` (a running `.exe` or
//! `.dll` can be renamed but not overwritten). If anything fails, moved files are
//! removed and backups restored, so the prefix is left as it was.
//!
//! Backup (configuration) files the user edited are never overwritten: the new
//! version lands next to them as `<file>.pacnew`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use paku_layout::Layout;

use crate::backup::{self, Action, BackupFile};
use crate::checksum::md5_file;
use crate::error::{Result, io_err};
use crate::install::conflict::matches_patterns;
use crate::remove::files::{Options, remove_files};

/// Suffix of files replaced during an install.
pub const OLD_SUFFIX: &str = ".paku-old";

/// What to install, and what the previously installed version left on disk.
#[derive(Debug, Clone, Copy, Default)]
pub struct Request<'a> {
    /// Staged files relative to the prefix; directories end with `/`.
    pub files: &'a [String],
    /// Backup files of the new version.
    pub backup: &'a [String],
    /// Files of the installed version, if any.
    pub old_files: &'a [String],
    /// Backup files of the installed version with their recorded hashes.
    pub old_backup: &'a [BackupFile],
    /// `NoUpgrade` patterns; matching files on disk are never replaced.
    pub no_upgrade: &'a [String],
}

/// Outcome of a successful commit.
#[derive(Debug, Default)]
pub struct Committed {
    /// Backups that could not be deleted (files in use); removed on a later run.
    pub leftovers: Vec<PathBuf>,
    /// The new version's backup files with their hashes as shipped.
    pub backup: Vec<BackupFile>,
    /// Files left untouched; the new version of each is at `<file>.pacnew`.
    pub pacnew: Vec<PathBuf>,
    /// Edited backup files the new version no longer has, and the `.pacsave`
    /// path each was renamed to.
    pub pacsave: Vec<(PathBuf, PathBuf)>,
}

#[derive(Default)]
struct Journal {
    moved: Vec<PathBuf>,
    backups: Vec<(PathBuf, PathBuf)>,
}

/// Installs the staged files into the prefix, then deletes files of the previous
/// version that the new version no longer has.
pub fn commit(staging: &Path, layout: &Layout, req: &Request) -> Result<Committed> {
    let mut journal = Journal::default();
    let mut done = Committed::default();
    if let Err(e) = move_all(staging, layout, req, &mut journal, &mut done) {
        rollback(&journal);
        return Err(e);
    }
    for (_, backup) in &journal.backups {
        if fs::remove_file(backup).is_err() {
            done.leftovers.push(backup.clone());
        }
    }
    remove_stale(layout, req, &mut done);
    Ok(done)
}

fn move_all(staging: &Path, layout: &Layout, req: &Request, journal: &mut Journal, done: &mut Committed) -> Result<()> {
    let staged = Layout::new(staging, layout.env());
    for rel in req.files {
        let dest = layout.install_path(rel);
        if rel.ends_with('/') {
            fs::create_dir_all(&dest).map_err(io_err(&dest))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let src = staged.install_path(rel);
        let Some(target) = target_path(req, rel, &src, dest, done)? else {
            continue;
        };
        if target.exists() {
            let backup = backup_path(&target);
            fs::rename(&target, &backup).map_err(io_err(&target))?;
            journal.backups.push((target.clone(), backup));
        }
        fs::rename(&src, &target).map_err(io_err(&target))?;
        journal.moved.push(target);
    }
    Ok(())
}

/// Where a staged file goes: its destination, `<destination>.pacnew`, or nowhere
/// (`None`) when the file on disk stays as it is.
fn target_path(req: &Request, rel: &str, src: &Path, dest: PathBuf, done: &mut Committed) -> Result<Option<PathBuf>> {
    let mut shipped = None;
    if req.backup.iter().any(|b| b.eq_ignore_ascii_case(rel)) {
        let hash = md5_file(src)?;
        done.backup.push(BackupFile {
            path: rel.to_string(),
            hash: hash.clone(),
        });
        shipped = Some(hash);
    }
    if !dest.is_file() {
        return Ok(Some(dest));
    }
    let action = if matches_patterns(req.no_upgrade, rel) {
        Action::Pacnew
    } else {
        backup_action(req, rel, src, &dest, shipped)?
    };
    Ok(match action {
        Action::Install => Some(dest),
        Action::Keep => None,
        Action::Pacnew => {
            let pacnew = backup::pacnew_path(&dest);
            done.pacnew.push(dest);
            Some(pacnew)
        }
    })
}

/// Decides between the file on disk and the staged one. Only backup files of the
/// old or the new version are compared; everything else is simply replaced.
fn backup_action(req: &Request, rel: &str, src: &Path, dest: &Path, shipped: Option<String>) -> Result<Action> {
    let orig = backup::find(req.old_backup, rel);
    if orig.is_none() && shipped.is_none() {
        return Ok(Action::Install);
    }
    let pkg = match shipped {
        Some(hash) => hash,
        None => md5_file(src)?,
    };
    let local = md5_file(dest)?;
    Ok(backup::decide(orig.map(|b| b.hash.as_str()), &local, &pkg))
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
pub(crate) fn backup_path(dest: &Path) -> PathBuf {
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
/// Edited backup files among them are saved as `.pacsave`.
fn remove_stale(layout: &Layout, req: &Request, done: &mut Committed) {
    let keep: HashSet<String> = req.files.iter().map(|f| f.to_ascii_lowercase()).collect();
    let stale: Vec<String> = req
        .old_files
        .iter()
        .filter(|f| !keep.contains(&f.to_ascii_lowercase()))
        .cloned()
        .collect();
    let opts = Options {
        keep_dirs: &HashSet::new(),
        backup: req.old_backup,
        nosave: false,
        no_upgrade: req.no_upgrade,
    };
    let removed = remove_files(layout, &stale, &opts);
    done.leftovers.extend(removed.leftovers);
    done.pacsave = removed.saved;
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

        let req = Request {
            files: &files(&["bin/", "bin/a.exe"]),
            old_files: &files(&["bin/", "bin/a.exe", "bin/gone.dll"]),
            ..Request::default()
        };
        let done = commit(&staging, &layout, &req).unwrap();
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
        let req = Request {
            files: &files(&["bin/a.exe", "bin/b.exe"]),
            ..Request::default()
        };
        assert!(commit(&staging, &layout, &req).is_err());
        assert_eq!(fs::read(prefix.join("bin/a.exe")).unwrap(), b"old");
        assert!(!prefix.join("bin/a.exe.paku-old").exists());
    }

    const MD5_V1: &str = "0cc175b9c0f1b6a831c399e269772661"; // md5("a")

    /// Upgrades a package whose etc/x.conf shipped as "a" to one shipping `new`,
    /// with `local` on disk. Returns the outcome and the prefix.
    fn upgrade_conf(dir: &Path, local: &[u8], new: &[u8], no_upgrade: &[&str]) -> (Committed, PathBuf) {
        let (staging, prefix) = (dir.join("s"), dir.join("p"));
        let layout = Layout::new(&prefix, "ucrt64");
        fs::create_dir_all(prefix.join("etc")).unwrap();
        fs::write(prefix.join("etc/x.conf"), local).unwrap();
        stage(&staging, "etc/x.conf", new);
        let list = files(&["etc/", "etc/x.conf"]);
        let req = Request {
            files: &list,
            backup: &files(&["etc/x.conf"]),
            old_files: &list,
            old_backup: &[BackupFile {
                path: "etc/x.conf".into(),
                hash: MD5_V1.into(),
            }],
            no_upgrade: &files(no_upgrade),
        };
        (commit(&staging, &layout, &req).unwrap(), prefix)
    }

    #[test]
    fn unedited_backup_file_is_upgraded() {
        let dir = tempdir();
        let (done, prefix) = upgrade_conf(dir.path(), b"a", b"b", &[]);
        assert!(done.pacnew.is_empty());
        assert_eq!(fs::read(prefix.join("etc/x.conf")).unwrap(), b"b");
        assert_eq!(done.backup.len(), 1);
        assert_eq!(done.backup[0].hash, "92eb5ffee6ae2fec3ad71c777531578f"); // md5("b")
    }

    #[test]
    fn edited_backup_file_is_kept_when_the_package_did_not_change_it() {
        let dir = tempdir();
        let (done, prefix) = upgrade_conf(dir.path(), b"mine", b"a", &[]);
        assert!(done.pacnew.is_empty());
        assert_eq!(fs::read(prefix.join("etc/x.conf")).unwrap(), b"mine");
        assert!(!prefix.join("etc/x.conf.pacnew").exists());
        assert_eq!(done.backup[0].hash, MD5_V1);
    }

    #[test]
    fn edited_backup_file_gets_a_pacnew_when_the_package_changed_it() {
        let dir = tempdir();
        let (done, prefix) = upgrade_conf(dir.path(), b"mine", b"b", &[]);
        assert_eq!(done.pacnew, [prefix.join("etc").join("x.conf")]);
        assert_eq!(fs::read(prefix.join("etc/x.conf")).unwrap(), b"mine");
        assert_eq!(fs::read(prefix.join("etc/x.conf.pacnew")).unwrap(), b"b");
    }

    #[test]
    fn noupgrade_file_is_never_replaced() {
        let dir = tempdir();
        let (done, prefix) = upgrade_conf(dir.path(), b"a", b"b", &["etc/*.conf"]);
        assert_eq!(done.pacnew.len(), 1);
        assert_eq!(fs::read(prefix.join("etc/x.conf")).unwrap(), b"a");
        assert_eq!(fs::read(prefix.join("etc/x.conf.pacnew")).unwrap(), b"b");
    }

    #[test]
    fn edited_backup_file_dropped_by_the_new_version_is_saved() {
        let dir = tempdir();
        let (staging, prefix) = (dir.path().join("s"), dir.path().join("p"));
        let layout = Layout::new(&prefix, "ucrt64");
        fs::create_dir_all(prefix.join("etc")).unwrap();
        fs::write(prefix.join("etc/x.conf"), b"mine").unwrap();
        stage(&staging, "bin/a.exe", b"new");
        let req = Request {
            files: &files(&["bin/", "bin/a.exe"]),
            old_files: &files(&["etc/", "etc/x.conf"]),
            old_backup: &[BackupFile {
                path: "etc/x.conf".into(),
                hash: MD5_V1.into(),
            }],
            ..Request::default()
        };
        let done = commit(&staging, &layout, &req).unwrap();
        assert_eq!(done.pacsave.len(), 1);
        assert_eq!(fs::read(prefix.join("etc/x.conf.pacsave")).unwrap(), b"mine");
        assert!(!prefix.join("etc/x.conf").exists());
    }
}
