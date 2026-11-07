// SPDX-License-Identifier: MIT

//! Native CPU architecture detection.

use std::fmt;

/// CPU architecture of the Windows host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
    /// x86-64 (AMD64).
    X64,
    /// ARM64 (AArch64).
    Arm64,
}

impl HostArch {
    /// Default MSYS2 mingw environment for this architecture.
    pub fn default_env(self) -> &'static str {
        match self {
            HostArch::X64 => "ucrt64",
            HostArch::Arm64 => "clangarm64",
        }
    }

    /// Architecture name as used in MSYS2 package metadata.
    pub fn msys2_name(self) -> &'static str {
        match self {
            HostArch::X64 => "x86_64",
            HostArch::Arm64 => "aarch64",
        }
    }
}

impl fmt::Display for HostArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.msys2_name())
    }
}

/// Detects the native machine architecture.
///
/// Uses `IsWow64Process2`, which reports the real CPU even when this process runs
/// under x64 emulation on ARM64 (where `uname -m` and `PROCESSOR_ARCHITECTURE`
/// report x86_64).
pub fn native_arch() -> HostArch {
    native_arch_impl().unwrap_or_else(compiled_arch)
}

/// Architecture this binary was compiled for.
pub fn compiled_arch() -> HostArch {
    if cfg!(target_arch = "aarch64") {
        HostArch::Arm64
    } else {
        HostArch::X64
    }
}

#[cfg(windows)]
fn native_arch_impl() -> Option<HostArch> {
    use crate::ffi::{GetCurrentProcess, IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64, IsWow64Process2};

    let mut process_machine = 0u16;
    let mut native_machine = 0u16;
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing; both
    // out-pointers reference valid stack locations.
    let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut process_machine, &mut native_machine) };
    if ok == 0 {
        return None;
    }
    match native_machine {
        IMAGE_FILE_MACHINE_ARM64 => Some(HostArch::Arm64),
        IMAGE_FILE_MACHINE_AMD64 => Some(HostArch::X64),
        _ => None,
    }
}

#[cfg(not(windows))]
fn native_arch_impl() -> Option<HostArch> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_envs() {
        assert_eq!(HostArch::X64.default_env(), "ucrt64");
        assert_eq!(HostArch::Arm64.default_env(), "clangarm64");
    }

    #[cfg(windows)]
    #[test]
    fn native_arch_is_detected() {
        assert!(native_arch_impl().is_some());
    }
}
