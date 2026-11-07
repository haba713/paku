// SPDX-License-Identifier: MIT

//! Minimal kernel32 bindings (avoids a `windows-sys` dependency).

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use core::ffi::c_void;

pub type Handle = *mut c_void;

pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
pub const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

pub const STD_INPUT_HANDLE: u32 = -10i32 as u32;
pub const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
pub const STD_ERROR_HANDLE: u32 = -12i32 as u32;
pub const FILE_TYPE_PIPE: u32 = 3;
/// `FILE_INFO_BY_HANDLE_CLASS::FileNameInfo`
pub const FILE_NAME_INFO_CLASS: i32 = 2;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetCurrentProcess() -> Handle;
    pub fn IsWow64Process2(process: Handle, process_machine: *mut u16, native_machine: *mut u16) -> i32;
    pub fn GetStdHandle(which: u32) -> Handle;
    pub fn GetFileType(file: Handle) -> u32;
    pub fn GetFileInformationByHandleEx(file: Handle, class: i32, info: *mut c_void, size: u32) -> i32;
    pub fn FileTimeToSystemTime(file_time: *const FILETIME, system_time: *mut SYSTEMTIME) -> i32;
    pub fn SystemTimeToTzSpecificLocalTime(
        tz: *const c_void,
        universal: *const SYSTEMTIME,
        local: *mut SYSTEMTIME,
    ) -> i32;
}
