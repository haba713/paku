// SPDX-License-Identifier: GPL-2.0-or-later

//! CNG calls behind [`super::RsaPublicKey::verify`].

use std::io;
use std::ptr;

use super::ffi::*;

/// Closes the algorithm provider and key handles on drop.
struct Handles {
    alg: Handle,
    key: Handle,
}

impl Drop for Handles {
    fn drop(&mut self) {
        // SAFETY: handles are either null or were returned by bcrypt and not yet released.
        unsafe {
            if !self.key.is_null() {
                BCryptDestroyKey(self.key);
            }
            if !self.alg.is_null() {
                BCryptCloseAlgorithmProvider(self.alg, 0);
            }
        }
    }
}

pub fn verify(modulus: &[u8], exponent: &[u8], hash_id: &str, digest: &[u8], sig: &[u8]) -> io::Result<bool> {
    let mut h = Handles {
        alg: ptr::null_mut(),
        key: ptr::null_mut(),
    };
    let rsa = wide("RSA");
    // SAFETY: valid out pointer and NUL-terminated algorithm name.
    check(unsafe { BCryptOpenAlgorithmProvider(&mut h.alg, rsa.as_ptr(), ptr::null(), 0) })?;
    let blob = key_blob(modulus, exponent)?;
    let blob_type = wide("RSAPUBLICBLOB");
    // SAFETY: blob is a complete BCRYPT_RSAKEY_BLOB of the given length.
    check(unsafe {
        BCryptImportKeyPair(
            h.alg,
            ptr::null_mut(),
            blob_type.as_ptr(),
            &mut h.key,
            blob.as_ptr(),
            len32(blob.len())?,
            0,
        )
    })?;
    let alg_id = wide(hash_id);
    let padding = BCRYPT_PKCS1_PADDING_INFO {
        pszAlgId: alg_id.as_ptr(),
    };
    // SAFETY: key handle is valid; buffers outlive the call.
    let status = unsafe {
        BCryptVerifySignature(
            h.key,
            (&padding as *const BCRYPT_PKCS1_PADDING_INFO).cast(),
            digest.as_ptr(),
            len32(digest.len())?,
            sig.as_ptr(),
            len32(sig.len())?,
            BCRYPT_PAD_PKCS1,
        )
    };
    match status {
        STATUS_SUCCESS => Ok(true),
        STATUS_INVALID_SIGNATURE => Ok(false),
        other => Err(ntstatus_error(other)),
    }
}

fn key_blob(modulus: &[u8], exponent: &[u8]) -> io::Result<Vec<u8>> {
    let header = BCRYPT_RSAKEY_BLOB {
        Magic: BCRYPT_RSAPUBLIC_MAGIC,
        BitLength: len32(modulus.len() * 8)?,
        cbPublicExp: len32(exponent.len())?,
        cbModulus: len32(modulus.len())?,
        cbPrime1: 0,
        cbPrime2: 0,
    };
    let mut blob = Vec::with_capacity(24 + exponent.len() + modulus.len());
    for field in [
        header.Magic,
        header.BitLength,
        header.cbPublicExp,
        header.cbModulus,
        header.cbPrime1,
        header.cbPrime2,
    ] {
        blob.extend_from_slice(&field.to_le_bytes());
    }
    blob.extend_from_slice(exponent);
    blob.extend_from_slice(modulus);
    Ok(blob)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn len32(n: usize) -> io::Result<u32> {
    u32::try_from(n).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "buffer too large"))
}

fn check(status: NTSTATUS) -> io::Result<()> {
    if status == STATUS_SUCCESS {
        Ok(())
    } else {
        Err(ntstatus_error(status))
    }
}

fn ntstatus_error(status: NTSTATUS) -> io::Error {
    io::Error::other(format!("CNG error 0x{:08X}", status as u32))
}
