// SPDX-License-Identifier: MIT

//! File formats used by MSYS2 package repositories and pacman-style databases.
//!
//! Everything here is written from the formats themselves (as found in the MSYS2
//! repositories), not from pacman's source.

pub mod archive;
pub mod depend;
pub mod desc;
pub mod meta;
pub mod package_versions;
pub mod pkgfile;
pub mod pkginfo;
pub mod syncdb;

/// Errors produced while reading package or database files.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Input did not match the expected format.
    #[error("{0}")]
    Invalid(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, FormatError>;
