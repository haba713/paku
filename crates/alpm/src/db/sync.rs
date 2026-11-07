// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Sync databases (`<dbpath>/sync/<repo>.db`), mirroring libalpm's `be_sync.c` role.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use paku_formats::meta::PkgMeta;
use paku_formats::syncdb;

use crate::download::{self, Fetched};
use crate::error::{Error, Result, io_err};

/// One configured repository and, once loaded, its package list.
#[derive(Debug, Clone)]
pub struct SyncDb {
    name: String,
    servers: Vec<String>,
    packages: Vec<PkgMeta>,
    by_name: HashMap<String, usize>,
}

impl SyncDb {
    /// Creates an unloaded repository with its server URLs (already expanded).
    pub fn new(name: impl Into<String>, servers: Vec<String>) -> Self {
        Self {
            name: name.into(),
            servers,
            packages: Vec::new(),
            by_name: HashMap::new(),
        }
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

    /// Downloads the database if the server has a newer one (or always with `force`).
    pub fn refresh(&self, dbpath: &Path, force: bool, opts: &download::Options) -> Result<Fetched> {
        if self.servers.is_empty() {
            return Err(Error::NoServers(self.name.clone()));
        }
        let file = format!("{}.db", self.name);
        download::fetch(&self.servers, &file, &self.path(dbpath), force, opts)
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

#[cfg(test)]
mod tests {
    use super::*;

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
