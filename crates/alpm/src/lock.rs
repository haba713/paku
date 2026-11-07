// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! The database lock file (`<dbpath>/db.lck`), as in libalpm's `_alpm_handle_lock`.

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result, io_err};

/// Held while a transaction modifies the install prefix; removed on drop.
#[derive(Debug)]
pub struct DbLock {
    path: PathBuf,
}

impl DbLock {
    /// Creates `<dbpath>/db.lck`, failing if another paku holds it.
    pub fn acquire(dbpath: &Path) -> Result<Self> {
        fs::create_dir_all(dbpath).map_err(io_err(dbpath))?;
        let path = dbpath.join("db.lck");
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(DbLock { path }),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(Error::Locked(path)),
            Err(e) => Err(io_err(&path)(e)),
        }
    }
}

impl Drop for DbLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_fails_until_released() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let lock = DbLock::acquire(dir.path()).unwrap();
        assert!(matches!(DbLock::acquire(dir.path()), Err(Error::Locked(_))));
        drop(lock);
        assert!(DbLock::acquire(dir.path()).is_ok());
    }
}
