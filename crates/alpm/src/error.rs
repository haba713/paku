// SPDX-License-Identifier: GPL-2.0-or-later

//! Library error type.

use std::path::PathBuf;

/// Errors returned by the library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure on a specific path.
    #[error("{path}: {source}")]
    Io {
        /// The path being accessed.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A database or package file could not be parsed.
    #[error(transparent)]
    Format(#[from] paku_formats::FormatError),
    /// The sync database has not been downloaded yet.
    #[error("database file for '{0}' does not exist (use '-Sy' to download)")]
    DbNotFound(String),
    /// All servers failed for a download.
    #[error("failed retrieving file '{file}' from all mirrors: {reason}")]
    Download {
        /// File name being downloaded.
        file: String,
        /// Last error seen.
        reason: String,
    },
    /// A repository has no `Server` configured.
    #[error("no servers configured for repository: {0}")]
    NoServers(String),
    /// Invalid search pattern.
    #[error("invalid regular expression '{0}'")]
    BadRegex(String),
    /// No package satisfies a dependency.
    #[error("unable to satisfy dependency '{dep}' required by {by}")]
    UnsatisfiedDep {
        /// The dependency string.
        dep: String,
        /// The package that needs it.
        by: String,
    },
    /// Another paku holds the database lock.
    #[error("could not lock database: {} exists", .0.display())]
    Locked(PathBuf),
    /// A downloaded package does not match the checksum in the database.
    #[error("{0} is invalid or corrupted (checksum mismatch)")]
    ChecksumMismatch(String),
    /// A package archive contains a path that must not be installed.
    #[error(transparent)]
    Layout(#[from] paku_layout::LayoutError),
    /// A package archive lacks or has an unusable `.PKGINFO`.
    #[error("{0}: missing or invalid .PKGINFO")]
    BadPackage(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Attaches a path to an I/O error.
pub(crate) fn io_err(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}
