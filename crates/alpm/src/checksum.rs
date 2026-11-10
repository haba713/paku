// SPDX-License-Identifier: GPL-2.0-or-later

//! File checksums.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use md5::Md5;
use sha2::{Digest, Sha256};

use crate::error::{Result, io_err};

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    digest_file::<Sha256>(path)
}

/// Lowercase hex MD5 of a file, as pacman records for backup files.
pub fn md5_file(path: &Path) -> Result<String> {
    digest_file::<Md5>(path)
}

fn digest_file<D: Digest>(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io_err(path))?;
    let mut hasher = D::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(io_err(path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digests() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        let f = dir.path().join("f");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(md5_file(&f).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    }
}
