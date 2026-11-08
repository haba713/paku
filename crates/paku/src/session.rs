// SPDX-License-Identifier: GPL-2.0-or-later

//! State shared by all operations: arguments, configuration and databases.

use std::fs;

use alpm::db::{GitProvided, LocalDb, SyncDb};
use alpm::download;
use alpm::signing::Keyring;

use crate::args::Args;
use crate::conf::Config;
use crate::ui::display::getcols;
use crate::ui::msg;

/// Everything an operation needs.
pub struct Session {
    pub args: Args,
    pub conf: Config,
    /// Output width for wrapping (0 = no wrapping).
    pub cols: usize,
    /// Configured repositories (loaded on demand with [`Session::load_sync_dbs`]).
    pub sync: Vec<SyncDb>,
    pub local: LocalDb,
    pub git: GitProvided,
}

impl Session {
    /// Opens the local database and prepares (unloaded) sync databases.
    pub fn open(args: Args, conf: Config) -> Result<Self, String> {
        let local = LocalDb::load(&conf.dbpath).map_err(|e| e.to_string())?;
        let git = conf.git_root.as_deref().map(GitProvided::load).unwrap_or_default();
        let sync = conf
            .repos
            .iter()
            .map(|r| SyncDb::new(r.name.clone(), conf.expanded_servers(r)).with_siglevel(conf.siglevel_for(r)))
            .collect();
        Ok(Session {
            args,
            conf,
            cols: getcols(),
            sync,
            local,
            git,
        })
    }

    /// Loads every sync database, warning about missing ones.
    /// Returns `false` if any database could not be read.
    pub fn load_sync_dbs(&mut self) -> bool {
        let mut ok = true;
        for db in &mut self.sync {
            if let Err(e) = db.load(&self.conf.dbpath) {
                match e {
                    alpm::Error::DbNotFound(_) => msg::warning(&e.to_string()),
                    _ => {
                        msg::error(&format!("could not open database '{}': {e}", db.name()));
                        ok = false;
                    }
                }
            }
        }
        ok
    }

    /// Installed version of `name`: paku's local database first, then Git's bundle.
    pub fn installed_version(&self, name: &str) -> Option<&str> {
        if let Some(p) = self.local.find(name) {
            return Some(&p.meta.version);
        }
        self.git.version_of(name)
    }

    /// Path relocation for the configured prefix and environment.
    pub fn layout(&self) -> paku_layout::Layout {
        paku_layout::Layout::new(self.conf.prefix.clone(), self.conf.env.clone())
    }

    /// Signing keys: `msys2.gpg` (and optional `msys2-revoked`) from `GPGDir` when
    /// present, else MSYS2's keyring embedded in paku. Prints an error on failure.
    pub fn keyring(&self) -> Option<Keyring> {
        let dir = &self.conf.gpgdir;
        let keys = dir.join("msys2.gpg");
        let loaded = if keys.is_file() {
            let revoked = fs::read_to_string(dir.join("msys2-revoked")).unwrap_or_default();
            fs::read(&keys)
                .map_err(|e| format!("{}: {e}", keys.display()))
                .and_then(|data| Keyring::load(&data, &revoked).map_err(|e| format!("{}: {e}", keys.display())))
        } else {
            Keyring::embedded().map_err(|e| e.to_string())
        };
        loaded
            .map_err(|e| msg::error(&format!("could not load keyring: {e}")))
            .ok()
    }

    /// Download settings from configuration and arguments.
    pub fn download_options(&self) -> download::Options {
        let defaults = download::Options::default();
        let timeout = if self.conf.disable_dl_timeout {
            None
        } else {
            defaults.timeout
        };
        download::Options { timeout, ..defaults }
    }
}
