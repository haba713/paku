// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of dump_pkg_full in src/pacman/package.c.

//! `-Si` / `-Qi` package information blocks.

use std::fmt::Write;

use alpm::PkgMeta;
use alpm::db::local::Reason;

use super::color::colors;
use super::display::{humanize_size, list_display, list_display_linebreak, string_display};

/// Where a package shown with `-i` comes from.
pub enum Origin<'a> {
    /// A sync repository.
    Sync { repo: &'a str },
    /// The local database.
    Local {
        reason: Reason,
        install_date: i64,
        has_scriptlet: bool,
    },
}

/// Field labels, padded like pacman's `make_aligned_titles`.
fn title(label: &str) -> String {
    const WIDTH: usize = 15; // "Compressed Size"
    format!("{label:<WIDTH$} :")
}

/// Appends the full information block for one package, ending with a blank line.
pub fn dump_pkg_full(out: &mut String, pkg: &PkgMeta, origin: &Origin, extra: bool, cols: usize) {
    if let Origin::Sync { repo } = origin {
        string_display(out, &title("Repository"), repo, cols);
    }
    string_display(out, &title("Name"), &pkg.name, cols);
    string_display(out, &title("Version"), &pkg.version, cols);
    string_display(out, &title("Description"), &pkg.desc, cols);
    string_display(out, &title("Architecture"), &pkg.arch, cols);
    string_display(out, &title("URL"), pkg.url.as_deref().unwrap_or_default(), cols);
    list_display(out, &title("Licenses"), &pkg.licenses, cols);
    list_display(out, &title("Groups"), &pkg.groups, cols);
    list_display(out, &title("Provides"), &pkg.provides, cols);
    list_display(out, &title("Depends On"), &pkg.depends, cols);
    list_display_linebreak(out, &title("Optional Deps"), &pkg.optdepends, cols);
    list_display(out, &title("Conflicts With"), &pkg.conflicts, cols);
    list_display(out, &title("Replaces"), &pkg.replaces, cols);
    sizes(out, pkg, origin);
    string_display(
        out,
        &title("Packager"),
        pkg.packager.as_deref().unwrap_or_default(),
        cols,
    );
    string_display(out, &title("Build Date"), &format_date(pkg.builddate), cols);
    local_fields(out, origin, cols);
    validation(out, pkg, origin, extra, cols);
    out.push('\n');
}

fn sizes(out: &mut String, pkg: &PkgMeta, origin: &Origin) {
    let c = colors();
    let mut unit = None;
    if let Origin::Sync { .. } = origin {
        let (size, label) = humanize_size(pkg.csize as i64, None);
        let _ = writeln!(
            out,
            "{}{}{} {size:.2} {label}",
            c.title,
            title("Download Size"),
            c.nocolor
        );
        unit = Some(label);
    }
    let (size, label) = humanize_size(pkg.isize as i64, unit);
    let _ = writeln!(
        out,
        "{}{}{} {size:.2} {label}",
        c.title,
        title("Installed Size"),
        c.nocolor
    );
}

fn local_fields(out: &mut String, origin: &Origin, cols: usize) {
    let Origin::Local {
        reason,
        install_date,
        has_scriptlet,
    } = origin
    else {
        return;
    };
    string_display(out, &title("Install Date"), &format_date(*install_date), cols);
    let reason = match reason {
        Reason::Explicit => "Explicitly installed",
        Reason::Depend => "Installed as a dependency for another package",
    };
    string_display(out, &title("Install Reason"), reason, cols);
    let script = if *has_scriptlet { "Yes" } else { "No" };
    string_display(out, &title("Install Script"), script, cols);
}

fn validation(out: &mut String, pkg: &PkgMeta, origin: &Origin, extra: bool, cols: usize) {
    if matches!(origin, Origin::Sync { .. }) && extra {
        string_display(out, &title("MD5 Sum"), "", cols);
        string_display(
            out,
            &title("SHA-256 Sum"),
            pkg.sha256.as_deref().unwrap_or_default(),
            cols,
        );
        let sigs = if pkg.pgpsig.is_some() { "Present" } else { "None" };
        list_display(out, &title("Signatures"), &[sigs.to_string()], cols);
        return;
    }
    let mut methods = Vec::new();
    if pkg.sha256.is_some() {
        methods.push("SHA-256 Sum".to_string());
    }
    if pkg.pgpsig.is_some() {
        methods.push("Signature".to_string());
    }
    if methods.is_empty() {
        methods.push("None".to_string());
    }
    list_display(out, &title("Validated By"), &methods, cols);
}

/// Formats a Unix timestamp like C-locale `strftime("%c")` in local time.
pub fn format_date(unix: i64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if unix <= 0 {
        return String::new();
    }
    let t = paku_platform::local_time(unix);
    format!(
        "{} {} {:>2} {:02}:{:02}:{:02} {}",
        DAYS[t.weekday as usize % 7],
        MONTHS[(t.month as usize + 11) % 12],
        t.day,
        t.hour,
        t.minute,
        t.second,
        t.year
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg() -> PkgMeta {
        PkgMeta {
            name: "ripgrep".into(),
            version: "14.1.1-1".into(),
            desc: "fast grep".into(),
            arch: "any".into(),
            csize: 2_000_000,
            isize: 6_000_000,
            depends: vec!["gcc-libs".into()],
            sha256: Some("ab".into()),
            ..PkgMeta::default()
        }
    }

    #[test]
    fn sync_info_block() {
        let mut out = String::new();
        dump_pkg_full(&mut out, &pkg(), &Origin::Sync { repo: "ucrt64" }, false, 0);
        assert!(out.starts_with("Repository      : ucrt64\nName            : ripgrep\n"));
        assert!(out.contains("Depends On      : gcc-libs\n"));
        assert!(out.contains("Optional Deps   : None\n"));
        // pacman only moves to the next unit above 2048; installed size reuses the download unit
        assert!(out.contains("Download Size   : 1953.12 KiB\n"), "{out}");
        assert!(out.contains("Installed Size  : 5859.38 KiB\n"), "{out}");
        assert!(out.contains("Validated By    : SHA-256 Sum\n"));
        assert!(out.ends_with("\n\n"));
    }

    #[test]
    fn local_info_block() {
        let mut out = String::new();
        let origin = Origin::Local {
            reason: Reason::Depend,
            install_date: 0,
            has_scriptlet: false,
        };
        dump_pkg_full(&mut out, &pkg(), &origin, false, 0);
        assert!(!out.contains("Repository"));
        assert!(out.contains("Install Reason  : Installed as a dependency for another package\n"));
    }

    #[test]
    fn date_format_shape() {
        let d = format_date(1_752_899_704);
        assert!(d.ends_with(" 2025"), "{d}");
        assert_eq!(d.len(), "Sat Jul 19 06:35:04 2025".len());
        assert_eq!(format_date(0), "");
    }
}
