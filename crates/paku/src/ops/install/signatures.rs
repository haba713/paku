// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the signature part of check_validity in lib/libalpm/sync.c.

//! Package signature checks between download and extraction.

use std::path::{Path, PathBuf};

use alpm::download::package_signature;
use alpm::resolve::Candidate;
use alpm::signing::{self, Keyring, SigError};

use crate::session::Session;
use crate::ui::msg;

/// Checks every downloaded package against its repository's `SigLevel`.
/// Prints pacman-style errors and returns `false` if any package fails.
pub fn verify(s: &Session, plan: &[Candidate], files: &[PathBuf]) -> bool {
    let Some(keyring) = s.keyring() else {
        return false;
    };
    let mut ok = true;
    for (c, path) in plan.iter().zip(files) {
        if let Err(e) = verify_one(s, &keyring, c, path) {
            msg::error(&format!("{}: {e}", c.pkg.name));
            ok = false;
        }
    }
    if !ok {
        msg::error("failed to commit transaction (invalid or corrupted package (PGP signature))");
    }
    ok
}

fn verify_one(s: &Session, keyring: &Keyring, c: &Candidate, path: &Path) -> Result<(), SigError> {
    let Some(db) = s.sync.iter().find(|db| db.name() == c.repo) else {
        return Ok(());
    };
    let level = db.siglevel().package;
    if level == signing::Check::Never {
        return Ok(());
    }
    let filename = c.pkg.filename.as_deref().unwrap_or_default();
    let cache = path.parent().unwrap_or(path);
    let opts = s.download_options();
    let sig = package_signature(db.servers(), filename, cache, c.pkg.pgpsig.as_deref(), &opts)?;
    signing::check_file(level, keyring, path, sig.as_deref()).map(|_| ())
}
