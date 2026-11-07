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

/// Tells the user how to put the prefix's `bin` on Git Bash's `PATH` via `~/.bashrc`.
/// Not the Windows user `PATH`: msys programs need Git's `usr/bin` DLLs, which only
/// Git Bash has on its `PATH`.
pub fn path_hint(bin: &Path, home: &Path) {
    if on_path(bin) {
        return;
    }
    let line = format!("export PATH=\"{}:$PATH\"", bash_path(bin, home));
    println!();
    println!("To use installed programs, add this line to ~/.bashrc and open a new Git Bash window:");
    println!("  {line}");
    println!("or run:");
    println!("  echo '{line}' >> ~/.bashrc");
}

/// `bin` as Git Bash writes it: `$HOME/...` under `home`, else `/c/...`.
fn bash_path(bin: &Path, home: &Path) -> String {
    if let Ok(rest) = bin.strip_prefix(home) {
        let rest = rest.display().to_string().replace('\\', "/");
        return format!("$HOME/{rest}");
    }
    let s = bin.display().to_string().replace('\\', "/");
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic() => {
            format!("/{}{}", drive.to_ascii_lowercase(), chars.as_str())
        }
        _ => s,
    }
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

    #[test]
    fn bash_paths() {
        let home = Path::new("C:\\Users\\me");
        assert_eq!(
            bash_path(Path::new("C:\\Users\\me\\.local\\bin"), home),
            "$HOME/.local/bin"
        );
        assert_eq!(bash_path(Path::new("D:\\tools\\bin"), home), "/d/tools/bin");
    }
}
