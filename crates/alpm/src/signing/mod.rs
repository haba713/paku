// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! OpenPGP signature checks for databases and packages (role of libalpm's `signing.c`).
//!
//! Only what MSYS2 uses is supported: v4 RSA keys and v4 binary-document
//! signatures over SHA-256/384/512. RSA itself runs in Windows CNG.

mod armor;
mod key;
mod keyring;
mod packet;
mod signature;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use paku_platform::rsa::RsaHash;
use sha2::{Digest, Sha256, Sha384, Sha512};

pub use armor::base64_decode;
pub use key::hex;
pub use keyring::{KeyEntry, Keyring};
pub use signature::Signature;

const SIG_BINARY_DOCUMENT: u8 = 0x00;
const RSA_SIGNING_ALGORITHMS: [u8; 2] = [1, 3];

/// Why a signature was not accepted. Messages follow pacman's wording.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SigError {
    /// No signature, but the signature level requires one.
    #[error("missing required signature")]
    Missing,
    /// The signing key is not in the keyring.
    #[error("key \"{0}\" is unknown")]
    UnknownKey(String),
    /// The signing key has been revoked.
    #[error("key \"{0}\" is revoked")]
    Revoked(String),
    /// The signature does not match the data.
    #[error("signature from \"{0}\" is invalid")]
    Invalid(String),
    /// A key or signature type paku cannot check.
    #[error("unsupported signature ({0})")]
    Unsupported(String),
    /// The signature data could not be parsed.
    #[error("invalid signature data")]
    Malformed,
    /// Reading the signed file failed.
    #[error("{0}")]
    Io(String),
}

/// How strictly signatures are checked (pacman's `Never` / `Optional` / `Required`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Check {
    /// Do not check signatures.
    Never,
    /// Check signatures that exist; accept unsigned files.
    Optional,
    /// Every file must have a valid signature.
    #[default]
    Required,
}

/// `SigLevel` split by what is checked. Defaults to `Required` for both, like
/// MSYS2's `pacman.conf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SigLevel {
    pub package: Check,
    pub database: Check,
}

/// Applies `level` to an optional detached signature over `path`.
///
/// Returns the signing key when a signature was checked, `None` when checking was
/// skipped (`Never`, or `Optional` without a signature).
pub fn check_file<'k>(
    level: Check,
    keyring: &'k Keyring,
    path: &Path,
    signature: Option<&[u8]>,
) -> Result<Option<&'k KeyEntry>, SigError> {
    match (level, signature) {
        (Check::Never, _) | (Check::Optional, None) => Ok(None),
        (Check::Required, None) => Err(SigError::Missing),
        (_, Some(sig)) => {
            let file = File::open(path).map_err(|e| SigError::Io(format!("{}: {e}", path.display())))?;
            verify(keyring, file, sig).map(Some)
        }
    }
}

/// Verifies a detached signature over everything `data` yields.
pub fn verify<'k>(keyring: &'k Keyring, data: impl Read, signature: &[u8]) -> Result<&'k KeyEntry, SigError> {
    let sig = Signature::from_detached(signature)?;
    if sig.sig_type != SIG_BINARY_DOCUMENT {
        return Err(SigError::Unsupported(format!("signature type 0x{:02x}", sig.sig_type)));
    }
    let hash = rsa_hash(sig.hash_algorithm)?;
    let key = signer(keyring, &sig)?;
    let (Some(rsa), true) = (&key.key.rsa, RSA_SIGNING_ALGORITHMS.contains(&sig.pub_algorithm)) else {
        return Err(SigError::Unsupported(format!(
            "public key algorithm {}",
            sig.pub_algorithm
        )));
    };
    let digest = digest(hash, data, &sig.trailer).map_err(|e| SigError::Io(e.to_string()))?;
    let [mpi] = sig.mpis.as_slice() else {
        return Err(SigError::Malformed);
    };
    if digest[..2] != sig.left16 {
        return Err(SigError::Invalid(key.display_name()));
    }
    match rsa.verify(hash, &digest, mpi) {
        Ok(true) => Ok(key),
        Ok(false) => Err(SigError::Invalid(key.display_name())),
        Err(e) => Err(SigError::Unsupported(e.to_string())),
    }
}

fn signer<'k>(keyring: &'k Keyring, sig: &Signature) -> Result<&'k KeyEntry, SigError> {
    let Some(key) = keyring.find(sig.issuer_fingerprint, sig.issuer_key_id) else {
        let id = sig
            .issuer_fingerprint
            .map(|f| hex(&f))
            .or_else(|| sig.issuer_key_id.map(|k| hex(&k)))
            .unwrap_or_default();
        return Err(SigError::UnknownKey(id));
    };
    if key.revoked {
        return Err(SigError::Revoked(key.display_name()));
    }
    Ok(key)
}

/// OpenPGP hash algorithm IDs 8, 9, 10. SHA-1 and older are refused.
fn rsa_hash(id: u8) -> Result<RsaHash, SigError> {
    match id {
        8 => Ok(RsaHash::Sha256),
        9 => Ok(RsaHash::Sha384),
        10 => Ok(RsaHash::Sha512),
        _ => Err(SigError::Unsupported(format!("hash algorithm {id}"))),
    }
}

fn digest(hash: RsaHash, data: impl Read, trailer: &[u8]) -> io::Result<Vec<u8>> {
    match hash {
        RsaHash::Sha256 => hash_with::<Sha256>(data, trailer),
        RsaHash::Sha384 => hash_with::<Sha384>(data, trailer),
        RsaHash::Sha512 => hash_with::<Sha512>(data, trailer),
    }
}

fn hash_with<D: Digest>(mut data: impl Read, trailer: &[u8]) -> io::Result<Vec<u8>> {
    let mut h = D::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = data.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    h.update(trailer);
    Ok(h.finalize().to_vec())
}

#[cfg(test)]
mod tests;
