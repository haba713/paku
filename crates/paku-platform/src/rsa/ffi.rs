// SPDX-License-Identifier: GPL-2.0-or-later

//! Minimal bcrypt (CNG) bindings for RSA signature verification.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use core::ffi::c_void;

pub type Handle = *mut c_void;
pub type NTSTATUS = i32;

pub const STATUS_SUCCESS: NTSTATUS = 0;
pub const STATUS_INVALID_SIGNATURE: NTSTATUS = 0xC000_A000_u32 as i32;
pub const BCRYPT_PAD_PKCS1: u32 = 0x2;
/// `BCRYPT_RSAPUBLIC_MAGIC` ("RSA1").
pub const BCRYPT_RSAPUBLIC_MAGIC: u32 = 0x3141_5352;

/// `BCRYPT_RSAKEY_BLOB` header; exponent and modulus (big-endian) follow it.
#[repr(C)]
pub struct BCRYPT_RSAKEY_BLOB {
    pub Magic: u32,
    pub BitLength: u32,
    pub cbPublicExp: u32,
    pub cbModulus: u32,
    pub cbPrime1: u32,
    pub cbPrime2: u32,
}

#[repr(C)]
pub struct BCRYPT_PKCS1_PADDING_INFO {
    pub pszAlgId: *const u16,
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    pub fn BCryptOpenAlgorithmProvider(
        algorithm: *mut Handle,
        alg_id: *const u16,
        implementation: *const u16,
        flags: u32,
    ) -> NTSTATUS;
    pub fn BCryptCloseAlgorithmProvider(algorithm: Handle, flags: u32) -> NTSTATUS;
    pub fn BCryptImportKeyPair(
        algorithm: Handle,
        import_key: Handle,
        blob_type: *const u16,
        key: *mut Handle,
        input: *const u8,
        input_len: u32,
        flags: u32,
    ) -> NTSTATUS;
    pub fn BCryptDestroyKey(key: Handle) -> NTSTATUS;
    pub fn BCryptVerifySignature(
        key: Handle,
        padding_info: *const c_void,
        hash: *const u8,
        hash_len: u32,
        signature: *const u8,
        signature_len: u32,
        flags: u32,
    ) -> NTSTATUS;
}
