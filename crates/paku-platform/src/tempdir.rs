// SPDX-License-Identifier: GPL-2.0-or-later

//! Self-deleting temporary directories (also used as install staging areas).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A directory removed recursively when dropped.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates a new uniquely named directory in the system temp directory.
    pub fn new(prefix: &str) -> io::Result<Self> {
        Self::new_in(&std::env::temp_dir(), prefix)
    }

    /// Creates a new uniquely named directory inside `parent`.
    pub fn new_in(parent: &Path, prefix: &str) -> io::Result<Self> {
        std::fs::create_dir_all(parent)?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        for _ in 0..100 {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let name = format!("{prefix}{}-{nanos:08x}-{n}", std::process::id());
            let path = parent.join(name);
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique temporary directory",
        ))
    }

    /// The directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_unique_and_removed() {
        let a = TempDir::new("paku-test-").unwrap();
        let b = TempDir::new("paku-test-").unwrap();
        assert_ne!(a.path(), b.path());
        std::fs::write(a.path().join("f"), b"x").unwrap();
        let kept = a.path().to_path_buf();
        drop(a);
        assert!(!kept.exists());
        assert!(b.path().is_dir());
    }
}
