// SPDX-License-Identifier: GPL-2.0-or-later

//! Version 4 public key packets (RFC 4880 §5.5.2) and their fingerprints (§12.2).

use paku_platform::rsa::RsaPublicKey;
use sha1::{Digest, Sha1};

use super::SigError;
use super::packet::Reader;

/// OpenPGP public key algorithm IDs for RSA (encrypt+sign, encrypt-only, sign-only).
const RSA_ALGORITHMS: [u8; 3] = [1, 2, 3];

/// A parsed public key or subkey.
#[derive(Debug, Clone)]
pub struct PublicKey {
    /// V4 fingerprint (SHA-1 over the key packet).
    pub fingerprint: [u8; 20],
    /// OpenPGP public key algorithm ID.
    pub algorithm: u8,
    /// Key material, for RSA keys only.
    pub rsa: Option<RsaPublicKey>,
}

impl PublicKey {
    /// Parses a public key or subkey packet body. Keys other than v4 are unsupported.
    pub fn parse(body: &[u8]) -> Result<Self, SigError> {
        let mut r = Reader::new(body);
        let version = r.u8()?;
        if version != 4 {
            return Err(SigError::Unsupported(format!("version {version} key")));
        }
        let _created = r.u32()?;
        let algorithm = r.u8()?;
        let rsa = if RSA_ALGORITHMS.contains(&algorithm) {
            let modulus = r.mpi()?.to_vec();
            let exponent = r.mpi()?.to_vec();
            Some(RsaPublicKey { modulus, exponent })
        } else {
            None
        };
        Ok(Self {
            fingerprint: fingerprint(body)?,
            algorithm,
            rsa,
        })
    }

    /// The 64-bit key ID: the low 8 bytes of the fingerprint.
    pub fn key_id(&self) -> [u8; 8] {
        let mut id = [0u8; 8];
        id.copy_from_slice(&self.fingerprint[12..]);
        id
    }
}

fn fingerprint(body: &[u8]) -> Result<[u8; 20], SigError> {
    let len = u16::try_from(body.len()).map_err(|_| SigError::Malformed)?;
    let mut h = Sha1::new();
    h.update([0x99]);
    h.update(len.to_be_bytes());
    h.update(body);
    let mut out = [0u8; 20];
    out.copy_from_slice(&h.finalize());
    Ok(out)
}

/// Uppercase hex, as gpg and pacman print fingerprints and key IDs.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::packet::{TAG_PUBLIC_KEY, packets};

    #[test]
    fn test_key_fingerprint() {
        let data = include_bytes!("../../testdata/signing/test-key.gpg");
        let packets = packets(data).unwrap();
        let primary = packets.iter().find(|p| p.tag == TAG_PUBLIC_KEY).unwrap();
        let key = PublicKey::parse(primary.body).unwrap();
        assert_eq!(hex(&key.fingerprint), "76537ED4218CABF125D559362E55F95E8F6A3B51");
        assert_eq!(hex(&key.key_id()), "2E55F95E8F6A3B51");
        assert_eq!(key.algorithm, 1);
        assert_eq!(key.rsa.unwrap().modulus.len(), 256);
    }
}
