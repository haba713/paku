// SPDX-License-Identifier: GPL-2.0-or-later

//! Packages bundled with Git for Windows, read from `/etc/package-versions.txt`.
//!
//! They count as installed for dependency resolution but are never upgraded or
//! removed by paku.

use std::path::{Path, PathBuf};

use paku_formats::package_versions;

/// Package names paku must never install, even if Git doesn't list them:
/// a second MSYS runtime or shell on `PATH` breaks every MSYS process.
/// Mirrors MSYS2's core-package list.
pub const CORE_PACKAGES: &[&str] = &[
    "bash",
    "filesystem",
    "mintty",
    "msys2-runtime",
    "msys2-runtime-devel",
    "pacman",
    "pacman-mirrors",
];

/// Whether `name` is an MSYS core package (see [`CORE_PACKAGES`]).
pub fn is_core_package(name: &str) -> bool {
    CORE_PACKAGES.contains(&name) || name.starts_with("msys2-runtime-")
}

/// Read-only set of packages provided by the Git for Windows installation.
#[derive(Debug, Clone, Default)]
pub struct GitProvided {
    packages: Vec<(String, String)>,
}

impl GitProvided {
    /// Loads `etc/package-versions.txt` below `git_root`; missing file means empty.
    pub fn load(git_root: &Path) -> Self {
        let path = Self::manifest_path(git_root);
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self::from_text(&text)
    }

    /// Parses manifest text.
    pub fn from_text(text: &str) -> Self {
        Self {
            packages: package_versions::parse(text),
        }
    }

    /// Location of the manifest inside a Git for Windows install.
    pub fn manifest_path(git_root: &Path) -> PathBuf {
        git_root.join("etc").join("package-versions.txt")
    }

    /// All `(name, version)` pairs.
    pub fn packages(&self) -> &[(String, String)] {
        &self.packages
    }

    /// Version of `name` if Git provides it.
    pub fn version_of(&self, name: &str) -> Option<&str> {
        self.packages.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_packages() {
        assert!(is_core_package("bash"));
        assert!(is_core_package("msys2-runtime-3.6"));
        assert!(!is_core_package("make"));
    }

    #[test]
    fn lookup() {
        let g = GitProvided::from_text("bash 5.3-1\ngrep 1~3.0-7\n");
        assert_eq!(g.version_of("grep"), Some("1~3.0-7"));
        assert_eq!(g.version_of("sed"), None);
    }
}
