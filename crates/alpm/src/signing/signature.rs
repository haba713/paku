// SPDX-License-Identifier: GPL-2.0-or-later

//! Version 4 signature packets (RFC 4880 §5.2.3).

use super::SigError;
use super::packet::{Reader, TAG_SIGNATURE, packets};

const SUBPACKET_ISSUER: u8 = 16;
const SUBPACKET_ISSUER_FINGERPRINT: u8 = 33;

/// A parsed v4 signature.
#[derive(Debug, Clone)]
pub struct Signature {
    /// Signature type (`0x00` = binary document, `0x20` = key revocation, ...).
    pub sig_type: u8,
    /// Public key algorithm ID.
    pub pub_algorithm: u8,
    /// Hash algorithm ID.
    pub hash_algorithm: u8,
    /// Leading 16 bits of the signed digest, for a quick mismatch check.
    pub left16: [u8; 2],
    /// Signature MPIs (one for RSA).
    pub mpis: Vec<Vec<u8>>,
    /// Issuer fingerprint (subpacket 33), if present.
    pub issuer_fingerprint: Option<[u8; 20]>,
    /// Issuer key ID (subpacket 16), if present.
    pub issuer_key_id: Option<[u8; 8]>,
    /// Bytes hashed after the data: version through hashed subpackets, plus the v4 trailer.
    pub trailer: Vec<u8>,
}

impl Signature {
    /// Parses the first signature packet of a detached signature (`.sig` / `PGPSIG`).
    pub fn from_detached(data: &[u8]) -> Result<Self, SigError> {
        let packets = packets(data)?;
        let packet = packets
            .iter()
            .find(|p| p.tag == TAG_SIGNATURE)
            .ok_or(SigError::Malformed)?;
        Self::parse(packet.body)
    }

    /// Parses a signature packet body. Signatures other than v4 are unsupported.
    pub fn parse(body: &[u8]) -> Result<Self, SigError> {
        let mut r = Reader::new(body);
        let version = r.u8()?;
        if version != 4 {
            return Err(SigError::Unsupported(format!("version {version} signature")));
        }
        let sig_type = r.u8()?;
        let pub_algorithm = r.u8()?;
        let hash_algorithm = r.u8()?;
        let hashed_len = usize::from(r.u16()?);
        let hashed = r.take(hashed_len)?;
        let trailer = v4_trailer(&body[..r.position()])?;
        let unhashed_len = usize::from(r.u16()?);
        let unhashed = r.take(unhashed_len)?;
        let left = r.take(2)?;
        let mut mpis = Vec::new();
        while !r.is_empty() {
            mpis.push(r.mpi()?.to_vec());
        }
        let mut sig = Self {
            sig_type,
            pub_algorithm,
            hash_algorithm,
            left16: [left[0], left[1]],
            mpis,
            issuer_fingerprint: None,
            issuer_key_id: None,
            trailer,
        };
        sig.read_issuer(hashed)?;
        sig.read_issuer(unhashed)?;
        Ok(sig)
    }

    fn read_issuer(&mut self, subpackets: &[u8]) -> Result<(), SigError> {
        let mut r = Reader::new(subpackets);
        while !r.is_empty() {
            let len = subpacket_length(&mut r)?;
            let data = r.take(len)?;
            let Some((&kind, value)) = data.split_first() else {
                return Err(SigError::Malformed);
            };
            match kind & 0x7f {
                SUBPACKET_ISSUER if value.len() == 8 && self.issuer_key_id.is_none() => {
                    self.issuer_key_id = value.try_into().ok();
                }
                // value = key version byte, then the fingerprint (20 bytes for v4)
                SUBPACKET_ISSUER_FINGERPRINT if value.len() == 21 && self.issuer_fingerprint.is_none() => {
                    self.issuer_fingerprint = value[1..].try_into().ok();
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn subpacket_length(r: &mut Reader) -> Result<usize, SigError> {
    let o1 = usize::from(r.u8()?);
    match o1 {
        0..=191 => Ok(o1),
        192..=254 => Ok(((o1 - 192) << 8) + usize::from(r.u8()?) + 192),
        _ => Ok(r.u32()? as usize),
    }
}

/// The hashed part of the packet followed by `0x04 0xFF` and its length (§5.2.4).
fn v4_trailer(hashed_part: &[u8]) -> Result<Vec<u8>, SigError> {
    let len = u32::try_from(hashed_part.len()).map_err(|_| SigError::Malformed)?;
    let mut t = hashed_part.to_vec();
    t.extend_from_slice(&[0x04, 0xFF]);
    t.extend_from_slice(&len.to_be_bytes());
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::key::hex;

    #[test]
    fn parses_gpg_detached_signature() {
        let sig = Signature::from_detached(include_bytes!("../../testdata/signing/data.txt.sha512.sig")).unwrap();
        assert_eq!(sig.sig_type, 0x00);
        assert_eq!(sig.pub_algorithm, 1);
        assert_eq!(sig.hash_algorithm, 10);
        assert_eq!(sig.mpis.len(), 1);
        assert_eq!(
            hex(&sig.issuer_fingerprint.unwrap()),
            "76537ED4218CABF125D559362E55F95E8F6A3B51"
        );
        assert_eq!(hex(&sig.issuer_key_id.unwrap()), "2E55F95E8F6A3B51");
        assert_eq!(
            &sig.trailer[sig.trailer.len() - 6..sig.trailer.len() - 4],
            &[0x04, 0xFF]
        );
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Signature::from_detached(b"not a signature").is_err());
        assert!(Signature::from_detached(&[]).is_err());
    }
}
