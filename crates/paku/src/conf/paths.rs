// SPDX-License-Identifier: GPL-2.0-or-later

//! Locating the user's home directory and the Git for Windows installation.

use std::env;
use std::path::{Path, PathBuf};

use alpm::db::GitProvided;

/// The user's home directory: `HOME` when it is a Windows path (Git Bash
/// converts it for native programs), else `USERPROFILE`.
pub fn home_dir() -> Option<PathBuf> {
    let from_home = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir());
    from_home.or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

/// Expands a leading `~/` (or `~\`) to `home`.
pub fn expand_tilde(value: &str, home: &Path) -> PathBuf {
    let rest = value.strip_prefix("~/").or_else(|| value.strip_prefix("~\\"));
    match rest {
        Some(rest) => home.join(rest),
        None if value == "~" => home.to_path_buf(),
        None => PathBuf::from(value),
    }
}

/// Finds the Git for Windows root by looking for `git.exe` on `PATH`
/// (`<root>/cmd`, `<root>/bin`, `<root>/<env>/bin`) and checking that the
/// root has a package manifest.
pub fn find_git_root() -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .filter(|dir| dir.join("git.exe").is_file())
        .flat_map(|dir| {
            dir.ancestors()
                .skip(1)
                .take(2)
                .map(Path::to_path_buf)
                .collect::<Vec<_>>()
        })
        .find(|root| GitProvided::manifest_path(root).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion() {
        let home = Path::new("C:/Users/me");
        assert_eq!(expand_tilde("~/x/y", home), home.join("x/y"));
        assert_eq!(expand_tilde("~", home), home);
        assert_eq!(expand_tilde("D:/abs", home), PathBuf::from("D:/abs"));
    }
}
