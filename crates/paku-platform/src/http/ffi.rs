// SPDX-License-Identifier: GPL-2.0-or-later

//! Minimal WinHTTP bindings.

#![allow(non_snake_case)]

use core::ffi::c_void;

pub type HInternet = *mut c_void;

pub const WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY: u32 = 4;
pub const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
pub const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
pub const WINHTTP_QUERY_LAST_MODIFIED: u32 = 11;
pub const WINHTTP_QUERY_CONTENT_LENGTH: u32 = 5;
pub const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
/// `(DWORD)-1`: header string is NUL-terminated.
pub const HEADERS_LENGTH_NUL_TERMINATED: u32 = u32::MAX;
pub const ERROR_WINHTTP_HEADER_NOT_FOUND: u32 = 12150;

#[link(name = "winhttp")]
unsafe extern "system" {
    pub fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> HInternet;
    pub fn WinHttpSetTimeouts(handle: HInternet, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
    pub fn WinHttpConnect(session: HInternet, server: *const u16, port: u16, reserved: u32) -> HInternet;
    pub fn WinHttpOpenRequest(
        connect: HInternet,
        verb: *const u16,
        object: *const u16,
        version: *const u16,
        referrer: *const u16,
        accept_types: *const *const u16,
        flags: u32,
    ) -> HInternet;
    pub fn WinHttpSendRequest(
        request: HInternet,
        headers: *const u16,
        headers_len: u32,
        optional: *const c_void,
        optional_len: u32,
        total_len: u32,
        context: usize,
    ) -> i32;
    pub fn WinHttpReceiveResponse(request: HInternet, reserved: *mut c_void) -> i32;
    pub fn WinHttpQueryHeaders(
        request: HInternet,
        info_level: u32,
        name: *const u16,
        buffer: *mut c_void,
        buffer_len: *mut u32,
        index: *mut u32,
    ) -> i32;
    pub fn WinHttpReadData(request: HInternet, buffer: *mut c_void, to_read: u32, read: *mut u32) -> i32;
    pub fn WinHttpCloseHandle(handle: HInternet) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetLastError() -> u32;
}
