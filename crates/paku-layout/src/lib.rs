// SPDX-License-Identifier: GPL-2.0-or-later

//! Relocation of MSYS2 package archive paths into a per-user install prefix.
//!
//! MSYS2 packages carry absolute-style paths rooted at the environment directory
//! (`ucrt64/bin/foo.exe`) or at `/usr` (`usr/bin/foo.exe`). paku strips that leading
//! component so the tree lands directly under the install prefix (`~/.local/bin/foo.exe`),
//! keeping the relative `bin`/`lib`/`share` shape that relocatable programs rely on.
//!
//! This crate is the only place that turns archive paths into install paths.

use std::path::{Path, PathBuf};

/// Top-level directory names that are stripped from archive paths, besides the
/// configured environment (`ucrt64`, `clangarm64`, ...).
const ALWAYS_STRIPPED: &[&str] = &["usr"];

/// Errors raised for archive paths that must never be installed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutError {
    /// The path is absolute, has a drive letter, or escapes the prefix with `..`.
    #[error("refusing unsafe path in package archive: {0}")]
    UnsafePath(String),
}

/// Where an archive entry goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mapped {
    /// Package metadata at the archive root (`.PKGINFO`, `.MTREE`, `.INSTALL`, ...).
    Metadata(String),
    /// The environment or `usr` directory itself; nothing to create.
    Root,
    /// A file or directory to install, relative to the prefix, `/`-separated.
    /// Directories keep a trailing `/` as in pacman file lists.
    Install(String),
}

/// Maps archive paths of one MSYS2 environment onto an install prefix.
#[derive(Debug, Clone)]
pub struct Layout {
    prefix: PathBuf,
    env: String,
}

impl Layout {
    /// Creates a layout installing into `prefix` for environment `env` (e.g. `ucrt64`).
    pub fn new(prefix: impl Into<PathBuf>, env: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            env: env.into(),
        }
    }

    /// The install prefix (e.g. the user's home directory).
    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// The MSYS2 environment name (e.g. `ucrt64`).
    pub fn env(&self) -> &str {
        &self.env
    }

    /// Classifies and relocates one archive entry path.
    pub fn map(&self, archive_path: &str) -> Result<Mapped, LayoutError> {
        let path = archive_path.strip_prefix("./").unwrap_or(archive_path);
        check_safe(path)?;
        let is_dir = path.ends_with('/');
        let mut parts = path.split('/').filter(|p| !p.is_empty() && *p != ".");
        let Some(first) = parts.next() else {
            return Ok(Mapped::Root);
        };
        let rest: Vec<&str> = parts.collect();
        if first.starts_with('.') && rest.is_empty() && !is_dir {
            return Ok(Mapped::Metadata(first.to_string()));
        }
        let stripped = self.is_stripped(first);
        if stripped && rest.is_empty() {
            return Ok(Mapped::Root);
        }
        let kept: Vec<&str> = if stripped {
            rest
        } else {
            std::iter::once(first).chain(rest).collect()
        };
        let mut rel = kept.join("/");
        if is_dir {
            rel.push('/');
        }
        Ok(Mapped::Install(rel))
    }

    /// Absolute filesystem path for a relocated, prefix-relative path.
    pub fn install_path(&self, rel: &str) -> PathBuf {
        let mut path = self.prefix.clone();
        path.extend(rel.split('/').filter(|p| !p.is_empty()));
        path
    }

    fn is_stripped(&self, first: &str) -> bool {
        first.eq_ignore_ascii_case(&self.env) || ALWAYS_STRIPPED.iter().any(|s| first.eq_ignore_ascii_case(s))
    }
}

/// Rejects absolute paths, drive letters, backslashes and `..` components.
fn check_safe(path: &str) -> Result<(), LayoutError> {
    let unsafe_path =
        path.starts_with('/') || path.contains('\\') || path.contains(':') || path.split('/').any(|p| p == "..");
    if unsafe_path {
        return Err(LayoutError::UnsafePath(path.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout::new("C:/Users/me", "ucrt64")
    }

    fn install(p: &str) -> Mapped {
        Mapped::Install(p.to_string())
    }

    #[test]
    fn strips_environment_prefix() {
        assert_eq!(layout().map("ucrt64/bin/rg.exe"), Ok(install("bin/rg.exe")));
        assert_eq!(layout().map("ucrt64/share/doc/"), Ok(install("share/doc/")));
    }

    #[test]
    fn strips_usr_for_msys_packages() {
        assert_eq!(layout().map("usr/bin/make.exe"), Ok(install("bin/make.exe")));
    }

    #[test]
    fn keeps_other_top_level_dirs() {
        assert_eq!(layout().map("etc/profile.d/x.sh"), Ok(install("etc/profile.d/x.sh")));
        assert_eq!(layout().map("clang64/bin/x.exe"), Ok(install("clang64/bin/x.exe")));
    }

    #[test]
    fn root_and_metadata_entries() {
        assert_eq!(layout().map("ucrt64/"), Ok(Mapped::Root));
        assert_eq!(layout().map("usr"), Ok(Mapped::Root));
        assert_eq!(layout().map(".PKGINFO"), Ok(Mapped::Metadata(".PKGINFO".into())));
        assert_eq!(layout().map("./.MTREE"), Ok(Mapped::Metadata(".MTREE".into())));
    }

    #[test]
    fn rejects_unsafe_paths() {
        for p in ["/etc/passwd", "ucrt64/../../x", "C:/x", "a\\b", ".."] {
            assert!(layout().map(p).is_err(), "{p} should be rejected");
        }
    }

    #[test]
    fn env_match_is_case_insensitive() {
        assert_eq!(layout().map("UCRT64/bin/a.dll"), Ok(install("bin/a.dll")));
    }

    #[test]
    fn install_path_joins_components() {
        let p = layout().install_path("bin/rg.exe");
        assert_eq!(p, Path::new("C:/Users/me").join("bin").join("rg.exe"));
    }
}
