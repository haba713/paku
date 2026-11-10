// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! The local database of installed packages (`<dbpath>/local/<name>-<version>/`),
//! mirroring libalpm's `be_local.c` layout.

use std::fs;
use std::path::{Path, PathBuf};

use paku_formats::desc::Desc;
use paku_formats::meta::PkgMeta;

use crate::backup::BackupFile;
use crate::error::{Result, io_err};

/// Local database schema version written to `ALPM_DB_VERSION`.
pub const DB_VERSION: u32 = 9;

/// Why a package is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Requested by the user.
    Explicit,
    /// Pulled in as a dependency.
    Depend,
}

/// One installed package.
#[derive(Debug, Clone)]
pub struct LocalPkg {
    /// Package metadata.
    pub meta: PkgMeta,
    /// Install reason.
    pub reason: Reason,
    /// Install date, seconds since the Unix epoch.
    pub install_date: i64,
    /// Installed files relative to the install prefix.
    pub files: Vec<String>,
    /// Backup (configuration) files with their hashes as shipped.
    pub backup: Vec<BackupFile>,
}

/// Installed packages read from disk.
#[derive(Debug, Clone, Default)]
pub struct LocalDb {
    packages: Vec<LocalPkg>,
}

impl LocalDb {
    /// Directory of the local database.
    pub fn dir(dbpath: &Path) -> PathBuf {
        dbpath.join("local")
    }

    /// Loads all installed packages; a missing database means nothing is installed.
    pub fn load(dbpath: &Path) -> Result<Self> {
        let dir = Self::dir(dbpath);
        if !dir.is_dir() {
            return Ok(Self::default());
        }
        let mut packages = Vec::new();
        for entry in fs::read_dir(&dir).map_err(io_err(&dir))? {
            let entry = entry.map_err(io_err(&dir))?;
            if let Some(pkg) = read_pkg(&entry.path())? {
                packages.push(pkg);
            }
        }
        packages.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
        Ok(Self { packages })
    }

    /// All installed packages sorted by name.
    pub fn packages(&self) -> &[LocalPkg] {
        &self.packages
    }

    /// Looks up an installed package by name.
    pub fn find(&self, name: &str) -> Option<&LocalPkg> {
        self.packages.iter().find(|p| p.meta.name == name)
    }
}

fn read_pkg(dir: &Path) -> Result<Option<LocalPkg>> {
    let desc_path = dir.join("desc");
    if !desc_path.is_file() {
        return Ok(None);
    }
    let desc = Desc::parse(&fs::read_to_string(&desc_path).map_err(io_err(&desc_path))?);
    let Some(meta) = PkgMeta::from_desc(&desc) else {
        return Ok(None);
    };
    let files_path = dir.join("files");
    let files_desc = fs::read_to_string(&files_path)
        .map(|text| Desc::parse(&text))
        .unwrap_or_default();
    let files = files_desc.get("FILES").to_vec();
    let backup = files_desc
        .get("BACKUP")
        .iter()
        .filter_map(|l| BackupFile::parse(l))
        .collect();
    let reason = match desc.first("REASON") {
        Some("1") => Reason::Depend,
        _ => Reason::Explicit,
    };
    let install_date = desc.first("INSTALLDATE").and_then(|d| d.parse().ok()).unwrap_or(0);
    Ok(Some(LocalPkg {
        meta,
        reason,
        install_date,
        files,
        backup,
    }))
}

/// A package to record in the local database.
#[derive(Debug, Clone, Copy)]
pub struct NewEntry<'a> {
    /// Package metadata (from `.PKGINFO`, enriched with the sync database's).
    pub meta: &'a PkgMeta,
    /// Install reason.
    pub reason: Reason,
    /// Install date, seconds since the Unix epoch.
    pub install_date: i64,
    /// Installed files relative to the prefix.
    pub files: &'a [String],
    /// Backup files with their hashes as shipped.
    pub backup: &'a [BackupFile],
    /// Raw `.MTREE`.
    pub mtree: Option<&'a [u8]>,
    /// Raw `.INSTALL` scriptlet.
    pub install: Option<&'a [u8]>,
}

/// Directory of one package's entry: `local/<name>-<version>` (`:` in versions is
/// stored as `~`, which Windows file names allow).
pub fn entry_dir(dbpath: &Path, name: &str, version: &str) -> PathBuf {
    LocalDb::dir(dbpath).join(format!("{name}-{}", version.replace(':', "~")))
}

/// Writes a package's `desc`, `files`, `mtree` and `install`, replacing any entry
/// for another version of the same package.
pub fn write_entry(dbpath: &Path, old_version: Option<&str>, entry: NewEntry) -> Result<()> {
    let dir = entry_dir(dbpath, &entry.meta.name, &entry.meta.version);
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    ensure_db_version(dbpath)?;
    let mut desc = entry.meta.to_local_desc();
    desc.set("INSTALLDATE", vec![entry.install_date.to_string()]);
    if entry.reason == Reason::Depend {
        desc.set("REASON", vec!["1".into()]);
    }
    desc.set("VALIDATION", vec!["sha256".into()]);
    let mut files = Desc::default();
    files.set("FILES", entry.files.to_vec());
    files.set("BACKUP", entry.backup.iter().map(BackupFile::to_line).collect());
    write(&dir.join("desc"), desc.to_text().as_bytes())?;
    write(&dir.join("files"), files.to_text().as_bytes())?;
    if let Some(m) = entry.mtree {
        write(&dir.join("mtree"), m)?;
    }
    if let Some(i) = entry.install {
        write(&dir.join("install"), i)?;
    }
    match old_version {
        Some(old) if old != entry.meta.version => remove_entry(dbpath, &entry.meta.name, old),
        _ => Ok(()),
    }
}

/// Deletes a package's entry directory.
pub fn remove_entry(dbpath: &Path, name: &str, version: &str) -> Result<()> {
    let dir = entry_dir(dbpath, name, version);
    match fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_err(&dir)(e)),
        _ => Ok(()),
    }
}

fn ensure_db_version(dbpath: &Path) -> Result<()> {
    let path = LocalDb::dir(dbpath).join("ALPM_DB_VERSION");
    if path.exists() {
        return Ok(());
    }
    write(&path, format!("{DB_VERSION}\n").as_bytes())
}

fn write(path: &Path, data: &[u8]) -> Result<()> {
    fs::write(path, data).map_err(io_err(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_missing() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        assert!(LocalDb::load(dir.path()).unwrap().packages().is_empty());
    }

    #[test]
    fn reads_installed_package() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let pkg = dir.path().join("local/foo-1-1");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("desc"), "%NAME%\nfoo\n\n%VERSION%\n1-1\n\n%REASON%\n1\n\n").unwrap();
        fs::write(pkg.join("files"), "%FILES%\nbin/\nbin/foo.exe\n\n").unwrap();

        let db = LocalDb::load(dir.path()).unwrap();
        let foo = db.find("foo").unwrap();
        assert_eq!(foo.reason, Reason::Depend);
        assert_eq!(foo.files, ["bin/", "bin/foo.exe"]);
    }

    #[test]
    fn write_then_read_and_upgrade_replaces_entry() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let files = vec!["bin/".to_string(), "bin/x.exe".to_string()];
        let mut meta = PkgMeta {
            name: "x".into(),
            version: "1~2.0-1".into(),
            ..PkgMeta::default()
        };
        let backup = vec![BackupFile {
            path: "etc/x.conf".into(),
            hash: "abc".into(),
        }];
        fn entry<'a>(meta: &'a PkgMeta, files: &'a [String], backup: &'a [BackupFile]) -> NewEntry<'a> {
            NewEntry {
                meta,
                reason: Reason::Depend,
                install_date: 42,
                files,
                backup,
                mtree: Some(b"m"),
                install: None,
            }
        }
        write_entry(dir.path(), None, entry(&meta, &files, &backup)).unwrap();
        meta.version = "1~2.1-1".into();
        write_entry(dir.path(), Some("1~2.0-1"), entry(&meta, &files, &backup)).unwrap();

        let db = LocalDb::load(dir.path()).unwrap();
        assert_eq!(db.packages().len(), 1);
        let x = db.find("x").unwrap();
        assert_eq!(
            (x.meta.version.as_str(), x.reason, x.install_date),
            ("1~2.1-1", Reason::Depend, 42)
        );
        assert_eq!(x.files, files);
        assert_eq!(x.backup, backup);
        assert!(dir.path().join("local/ALPM_DB_VERSION").is_file());
    }
}
