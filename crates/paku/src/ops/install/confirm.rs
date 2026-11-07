// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of display_targets in src/pacman/util.c.

//! The package list, sizes and confirmation before installing.

use std::fmt::Write;

use alpm::resolve::Candidate;

use crate::session::Session;
use crate::ui::color::colors;
use crate::ui::display::{humanize_size, list_display};
use crate::ui::prompt::yesno;

/// Prints `Packages (N) ...` and the totals, then asks to proceed.
pub fn confirm(s: &Session, plan: &[Candidate]) -> bool {
    let mut out = String::new();
    let names: Vec<String> = plan
        .iter()
        .map(|c| format!("{}-{}", c.pkg.name, c.pkg.version))
        .collect();
    out.push('\n');
    list_display(&mut out, &format!("Packages ({})", plan.len()), &names, s.cols);
    out.push('\n');
    let download: u64 = plan.iter().filter(|c| !is_cached(s, c)).map(|c| c.pkg.csize).sum();
    let installed: u64 = plan.iter().map(|c| c.pkg.isize).sum();
    if download > 0 {
        size_line(&mut out, "Total Download Size:", download);
    }
    size_line(&mut out, "Total Installed Size:", installed);
    println!("{out}");
    yesno("Proceed with installation?", s.args.noconfirm == Some(true))
}

fn size_line(out: &mut String, label: &str, bytes: u64) {
    let c = colors();
    let (value, unit) = humanize_size(bytes as i64, None);
    let _ = writeln!(out, "{}{label:<22}{} {value:.2} {unit}", c.title, c.nocolor);
}

/// A package file already in the cache (checksum is verified later).
pub fn is_cached(s: &Session, c: &Candidate) -> bool {
    let Some(file) = &c.pkg.filename else {
        return false;
    };
    s.conf.cachedirs.iter().any(|d| d.join(file).is_file())
}

/// `-p`: prints download URLs (or `--print-format`) instead of installing.
pub fn print_targets(s: &Session, plan: &[Candidate]) {
    for c in plan {
        let url = server_url(s, c).unwrap_or_default();
        let line = match &s.args.print_format {
            Some(fmt) => fmt
                .replace("%n", &c.pkg.name)
                .replace("%v", &c.pkg.version)
                .replace("%r", c.repo)
                .replace("%a", &c.pkg.arch)
                .replace("%s", &c.pkg.csize.to_string())
                .replace("%l", &url)
                .replace("%d", &c.pkg.desc),
            None => url,
        };
        println!("{line}");
    }
}

/// First server URL for the package file.
pub fn server_url(s: &Session, c: &Candidate) -> Option<String> {
    let db = s.sync.iter().find(|db| db.name() == c.repo)?;
    let server = db.servers().first()?;
    Some(format!(
        "{}/{}",
        server.trim_end_matches('/'),
        c.pkg.filename.as_deref()?
    ))
}
