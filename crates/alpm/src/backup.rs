// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the backup handling in extract_single_file (lib/libalpm/add.c) and
// unlink_file / shift_pacsave (lib/libalpm/remove.c).

//! Backup files: configuration files a package marks with `backup = ...`, which
//! must survive upgrades and removals when the user has edited them.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Result, io_err};

/// Suffix of a new config file installed next to an edited one.
pub const PACNEW: &str = ".pacnew";
/// Suffix of an edited config file kept after its package is removed.
pub const PACSAVE: &str = ".pacsave";

/// A backup file of an installed package, as stored in `%BACKUP%`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    /// Path relative to the install prefix.
    pub path: String,
    /// Lowercase hex MD5 of the file as the package shipped it.
    pub hash: String,
}

impl BackupFile {
    /// Parses a `path<TAB>hash` line; lines without a hash are dropped.
    pub fn parse(line: &str) -> Option<Self> {
        let (path, hash) = line.split_once('\t')?;
        Some(BackupFile {
            path: path.to_string(),
            hash: hash.to_string(),
        })
    }

    /// The `path<TAB>hash` line.
    pub fn to_line(&self) -> String {
        format!("{}\t{}", self.path, self.hash)
    }
}

/// The entry for `path` (paths compare case-insensitively, as on disk).
pub fn find<'a>(list: &'a [BackupFile], path: &str) -> Option<&'a BackupFile> {
    list.iter().find(|b| b.path.eq_ignore_ascii_case(path))
}

/// What to do with an incoming backup file whose destination already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Replace the file on disk.
    Install,
    /// Leave the file on disk alone and drop the incoming one.
    Keep,
    /// Leave the file on disk alone and install the incoming one as `.pacnew`.
    Pacnew,
}

/// Compares the hash recorded at install time (`orig`), the file on disk (`local`)
/// and the incoming file (`pkg`), like pacman.
pub fn decide(orig: Option<&str>, local: &str, pkg: &str) -> Action {
    if local == pkg {
        return Action::Install;
    }
    match orig {
        // the package did not change the file: keep the user's edits
        Some(o) if o == pkg => Action::Keep,
        // the user did not change the file: take the new version
        Some(o) if o == local => Action::Install,
        _ => Action::Pacnew,
    }
}

/// `<file>.pacnew`.
pub fn pacnew_path(file: &Path) -> PathBuf {
    suffixed(file, PACNEW)
}

/// Renames `file` to `<file>.pacsave`; older saves move to `.pacsave.1`, `.pacsave.2`, ...
pub fn save(file: &Path) -> Result<PathBuf> {
    shift_pacsave(file);
    let saved = suffixed(file, PACSAVE);
    fs::rename(file, &saved).map_err(io_err(file))?;
    Ok(saved)
}

/// Makes room for a new `.pacsave` by shifting existing ones up by one.
fn shift_pacsave(file: &Path) {
    let numbered = |n: u32| suffixed(file, &format!("{PACSAVE}.{n}"));
    let max = (1..).take_while(|&n| numbered(n).exists()).last().unwrap_or(0);
    for n in (1..=max).rev() {
        let _ = fs::rename(numbered(n), numbered(n + 1));
    }
    let current = suffixed(file, PACSAVE);
    if current.exists() {
        let _ = fs::rename(current, numbered(1));
    }
}

fn suffixed(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_match_pacman() {
        // unchanged by the user: upgrade
        assert_eq!(decide(Some("a"), "a", "b"), Action::Install);
        // edited by the user, unchanged in the package: keep
        assert_eq!(decide(Some("a"), "x", "a"), Action::Keep);
        // edited by the user and changed in the package: .pacnew
        assert_eq!(decide(Some("a"), "x", "b"), Action::Pacnew);
        // already identical to the new file
        assert_eq!(decide(Some("a"), "b", "b"), Action::Install);
        // no recorded hash (file became a backup file, or was untracked)
        assert_eq!(decide(None, "x", "b"), Action::Pacnew);
        assert_eq!(decide(None, "b", "b"), Action::Install);
    }

    #[test]
    fn lines_round_trip_and_lookup_ignores_case() {
        let b = BackupFile::parse("etc/foo.conf\tabc").unwrap();
        assert_eq!(b.to_line(), "etc/foo.conf\tabc");
        assert!(BackupFile::parse("etc/foo.conf").is_none());
        assert!(find(&[b], "ETC/Foo.conf").is_some());
    }

    #[test]
    fn save_shifts_older_saves() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let file = dir.path().join("a.conf");
        for content in ["one", "two", "three"] {
            fs::write(&file, content).unwrap();
            assert_eq!(save(&file).unwrap(), dir.path().join("a.conf.pacsave"));
        }
        let read = |name: &str| fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(read("a.conf.pacsave"), "three");
        assert_eq!(read("a.conf.pacsave.1"), "two");
        assert_eq!(read("a.conf.pacsave.2"), "one");
        assert!(!file.exists());
    }
}
