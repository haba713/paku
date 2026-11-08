// SPDX-License-Identifier: GPL-2.0-or-later

//! RSA PKCS#1 v1.5 signature verification with Windows CNG (`bcrypt.dll`).
//!
//! Callers hash the data themselves; this only checks the signature over a digest.

#[cfg(windows)]
mod ffi;
#[cfg(windows)]
mod win;

use std::io;

/// Digest algorithm the signature was made over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsaHash {
    Sha256,
    Sha384,
    Sha512,
}

impl RsaHash {
    /// CNG algorithm identifier.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn cng_id(self) -> &'static str {
        match self {
            RsaHash::Sha256 => "SHA256",
            RsaHash::Sha384 => "SHA384",
            RsaHash::Sha512 => "SHA512",
        }
    }
}

/// An RSA public key: big-endian modulus and public exponent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsaPublicKey {
    pub modulus: Vec<u8>,
    pub exponent: Vec<u8>,
}

impl RsaPublicKey {
    /// Checks a PKCS#1 v1.5 `signature` over `digest`. `Ok(false)` means the
    /// signature does not match; `Err` means the key or input was unusable.
    pub fn verify(&self, hash: RsaHash, digest: &[u8], signature: &[u8]) -> io::Result<bool> {
        let modulus = strip_zeros(&self.modulus);
        let exponent = strip_zeros(&self.exponent);
        if modulus.is_empty() || exponent.is_empty() || signature.len() > modulus.len() {
            return Ok(false);
        }
        // OpenPGP strips leading zero bytes; CNG wants the signature as long as the modulus.
        let mut sig = vec![0u8; modulus.len() - signature.len()];
        sig.extend_from_slice(signature);
        verify_impl(modulus, exponent, hash, digest, &sig)
    }
}

fn strip_zeros(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|&x| x != 0).unwrap_or(b.len());
    &b[start..]
}

#[cfg(windows)]
fn verify_impl(modulus: &[u8], exponent: &[u8], hash: RsaHash, digest: &[u8], sig: &[u8]) -> io::Result<bool> {
    win::verify(modulus, exponent, hash.cng_id(), digest, sig)
}

#[cfg(not(windows))]
fn verify_impl(_: &[u8], _: &[u8], _: RsaHash, _: &[u8], _: &[u8]) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "RSA verification needs Windows",
    ))
}
