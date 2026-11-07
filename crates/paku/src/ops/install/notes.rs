// SPDX-License-Identifier: GPL-2.0-or-later

//! Messages after installing: optional dependencies, msys caveats, `PATH` hint.

use std::path::Path;

use alpm::PkgMeta;

use crate::ui::color::colors;

/// `Optional dependencies for <pkg>` with each `name: reason`, as pacman prints them.
pub fn optdepends(meta: &PkgMeta) {
    if meta.optdepends.is_empty() {
        return;
    }
    let c = colors();
    println!("{}Optional dependencies for {}{}", c.title, meta.name, c.nocolor);
    for o in &meta.optdepends {
        println!("    {o}");
    }
}

/// msys programs resolve `/usr/...` and `/etc/...` against Git's installation, so
/// data files installed under the prefix may not be found. Docs don't matter.
pub fn msys_caveat(repo: &str, name: &str, files: &[String]) {
    const HARMLESS: &[&str] = &[
        "bin/",
        "share/doc/",
        "share/licenses/",
        "share/man/",
        "share/info/",
        "share/locale/",
    ];
    if repo != "msys" {
        return;
    }
    let affected = files
        .iter()
        .filter(|f| !f.ends_with('/'))
        .any(|f| !HARMLESS.iter().any(|h| f.starts_with(h)));
    if affected {
        println!(
            "note: {name} is an msys package; if it looks for data under /usr or /etc, it searches \
             Git for Windows' folder, not ~/.local"
        );
    }
}

/// Tells the user how to put the prefix's `bin` on the Windows user `PATH`.
pub fn path_hint(bin: &Path) {
    if on_path(bin) {
        return;
    }
    let win = bin.display().to_string().replace('/', "\\");
    println!();
    println!("To use installed programs, add {win} to your Windows user PATH:");
    println!("  1. Start menu: open \"Edit environment variables for your account\".");
    println!("  2. Select \"Path\", click Edit, then New, and enter the directory above.");
    println!("  3. Click OK twice and open a new Git Bash window.");
}

fn on_path(bin: &Path) -> bool {
    let want = normalize(&bin.display().to_string());
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| normalize(&d.display().to_string()) == want))
        .unwrap_or(false)
}

fn normalize(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_normalization() {
        assert_eq!(
            normalize("C:/Users/Me/.local/bin/"),
            normalize("c:\\users\\me\\.local\\bin")
        );
    }
}
