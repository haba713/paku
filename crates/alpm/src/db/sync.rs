// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Sync databases (`<dbpath>/sync/<repo>.db`), mirroring libalpm's `be_sync.c` role.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use paku_formats::meta::PkgMeta;
use paku_formats::syncdb;

use crate::download::{self, Fetched};
use crate::error::{Error, Result, io_err};
use crate::signing::{self, Check, Keyring, SigError, SigLevel};

/// One configured repository and, once loaded, its package list.
#[derive(Debug, Clone)]
pub struct SyncDb {
    name: String,
    servers: Vec<String>,
    siglevel: SigLevel,
    packages: Vec<PkgMeta>,
    by_name: HashMap<String, usize>,
}

impl SyncDb {
    /// Creates an unloaded repository with its server URLs (already expanded)
    /// and the default signature level.
    pub fn new(name: impl Into<String>, servers: Vec<String>) -> Self {
        Self {
            name: name.into(),
            servers,
            siglevel: SigLevel::default(),
            packages: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    /// Sets the signature level for this repository's database and packages.
    pub fn with_siglevel(mut self, siglevel: SigLevel) -> Self {
        self.siglevel = siglevel;
        self
    }

    /// Signature level for this repository's database and packages.
    pub fn siglevel(&self) -> SigLevel {
        self.siglevel
    }

    /// Repository name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Server base URLs, in priority order.
    pub fn servers(&self) -> &[String] {
        &self.servers
    }

    /// Path of the downloaded database file.
    pub fn path(&self, dbpath: &Path) -> PathBuf {
        dbpath.join("sync").join(format!("{}.db", self.name))
    }

    /// Downloads the database if the server has a newer one (or always with `force`),
    /// then checks its detached signature (`<repo>.db.sig`) against `keyring`.
    /// A database that fails the check is deleted together with its signature.
    pub fn refresh(&self, dbpath: &Path, force: bool, opts: &download::Options, keyring: &Keyring) -> Result<Fetched> {
        if self.servers.is_empty() {
            return Err(Error::NoServers(self.name.clone()));
        }
        let path = self.path(dbpath);
        let file = format!("{}.db", self.name);
        let outcome = download::fetch(&self.servers, &file, &path, force, opts)?;
        let refetch = force || outcome == Fetched::Updated;
        if let Err(source) = self.check_signature(&path, refetch, opts, keyring) {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(sig_path(&path));
            return Err(Error::Signature {
                name: self.name.clone(),
                source,
            });
        }
        Ok(outcome)
    }

    /// Fetches `<repo>.db.sig` (conditionally unless the database changed) and checks it.
    /// A failed signature download falls back to an existing local signature, which is
    /// removed first whenever the database itself was replaced.
    fn check_signature(
        &self,
        db: &Path,
        refetch: bool,
        opts: &download::Options,
        keyring: &Keyring,
    ) -> std::result::Result<(), SigError> {
        if self.siglevel.database == Check::Never {
            return Ok(());
        }
        let sig = sig_path(db);
        if refetch {
            let _ = fs::remove_file(&sig);
        }
        let file = format!("{}.db.sig", self.name);
        let _ = download::fetch(&self.servers, &file, &sig, refetch, opts);
        let data = fs::read(&sig).ok();
        signing::check_file(self.siglevel.database, keyring, db, data.as_deref()).map(|_| ())
    }

    /// Loads the package list from the downloaded database file.
    pub fn load(&mut self, dbpath: &Path) -> Result<()> {
        let path = self.path(dbpath);
        if !path.exists() {
            return Err(Error::DbNotFound(self.name.clone()));
        }
        let file = File::open(&path).map_err(io_err(&path))?;
        let entries = syncdb::read(std::io::BufReader::new(file))?;
        let packages = entries.iter().filter_map(|e| PkgMeta::from_desc(&e.desc));
        self.set_packages(packages.collect());
        Ok(())
    }

    /// Replaces the package list (also used by tests).
    pub fn set_packages(&mut self, packages: Vec<PkgMeta>) {
        self.by_name = packages.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect();
        self.packages = packages;
    }

    /// All packages, in database order.
    pub fn packages(&self) -> &[PkgMeta] {
        &self.packages
    }

    /// Looks up a package by exact name.
    pub fn find(&self, name: &str) -> Option<&PkgMeta> {
        self.by_name.get(name).map(|&i| &self.packages[i])
    }
}

fn sig_path(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(".sig");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &[u8] = include_bytes!("../../testdata/signing/data.txt");
    const SIG: &[u8] = include_bytes!("../../testdata/signing/data.txt.sha512.sig");
    const KEY: &[u8] = include_bytes!("../../testdata/signing/test-key.asc");

    /// A `file://` repo serving `r.db` (the fixture data) and optionally `r.db.sig`.
    fn signed_repo(sig: Option<&[u8]>) -> (paku_platform::TempDir, paku_platform::TempDir, SyncDb) {
        let repo = paku_platform::TempDir::new("paku-test-").unwrap();
        let dbpath = paku_platform::TempDir::new("paku-test-").unwrap();
        fs::write(repo.path().join("r.db"), DATA).unwrap();
        if let Some(sig) = sig {
            fs::write(repo.path().join("r.db.sig"), sig).unwrap();
        }
        let db = SyncDb::new("r", vec![format!("file://{}", repo.path().display())]);
        (repo, dbpath, db)
    }

    fn refresh(db: &SyncDb, dbpath: &Path) -> Result<Fetched> {
        let ring = Keyring::load(KEY, "").unwrap();
        db.refresh(dbpath, false, &download::Options::default(), &ring)
    }

    #[test]
    fn refresh_accepts_valid_signature() {
        let (_repo, dbpath, db) = signed_repo(Some(SIG));
        assert_eq!(refresh(&db, dbpath.path()).unwrap(), Fetched::Updated);
        assert!(dbpath.path().join("sync/r.db.sig").is_file());
        assert_eq!(refresh(&db, dbpath.path()).unwrap(), Fetched::UpToDate);
    }

    #[test]
    fn refresh_rejects_bad_signature_and_removes_db() {
        let mut bad = SIG.to_vec();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        let (_repo, dbpath, db) = signed_repo(Some(&bad));
        let err = refresh(&db, dbpath.path()).unwrap_err();
        assert!(matches!(
            err,
            Error::Signature {
                source: SigError::Invalid(_),
                ..
            }
        ));
        assert!(!dbpath.path().join("sync/r.db").exists());
        assert!(!dbpath.path().join("sync/r.db.sig").exists());
    }

    #[test]
    fn refresh_missing_signature_by_level() {
        let (_repo, dbpath, db) = signed_repo(None);
        let err = refresh(&db, dbpath.path()).unwrap_err();
        assert!(matches!(
            err,
            Error::Signature {
                source: SigError::Missing,
                ..
            }
        ));

        let optional = SigLevel {
            database: Check::Optional,
            ..SigLevel::default()
        };
        let db = db.with_siglevel(optional);
        assert_eq!(refresh(&db, dbpath.path()).unwrap(), Fetched::Updated);
    }

    #[test]
    fn missing_db_is_reported() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let mut db = SyncDb::new("ucrt64", vec![]);
        assert!(matches!(db.load(dir.path()), Err(Error::DbNotFound(_))));
    }

    #[test]
    fn loads_db_file() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        std::fs::create_dir_all(dir.path().join("sync")).unwrap();
        let mut b = tar::Builder::new(Vec::new());
        let desc = "%NAME%\nfoo\n\n%VERSION%\n1-1\n\n";
        let mut h = tar::Header::new_gnu();
        h.set_size(desc.len() as u64);
        h.set_cksum();
        b.append_data(&mut h, "foo-1-1/desc", desc.as_bytes()).unwrap();
        std::fs::write(dir.path().join("sync/r.db"), b.into_inner().unwrap()).unwrap();

        let mut db = SyncDb::new("r", vec![]);
        db.load(dir.path()).unwrap();
        assert_eq!(db.find("foo").unwrap().version, "1-1");
        assert!(db.find("bar").is_none());
    }
}
