// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of process_siglevel / merge_siglevel in src/pacman/conf.c.

//! `SigLevel` values: `Never`, `Optional`, `Required`, optionally prefixed with
//! `Package` or `Database`. `TrustedOnly` / `TrustAll` are accepted; paku trusts
//! every non-revoked key in its keyring.

use alpm::signing::{Check, SigLevel};

/// The parts of a signature level that config lines have set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SigLevelSetting {
    pub package: Option<Check>,
    pub database: Option<Check>,
}

impl SigLevelSetting {
    /// Applies one `SigLevel = ...` value; later tokens override earlier ones.
    /// On error, returns the offending token.
    pub fn apply(&mut self, value: &str) -> Result<(), String> {
        for token in value.split_whitespace() {
            let (package, database, word) = if let Some(w) = token.strip_prefix("Package") {
                (true, false, w)
            } else if let Some(w) = token.strip_prefix("Database") {
                (false, true, w)
            } else {
                (true, true, token)
            };
            let check = match word {
                "Never" => Check::Never,
                "Optional" => Check::Optional,
                "Required" => Check::Required,
                "TrustedOnly" | "TrustAll" => continue,
                _ => return Err(token.to_string()),
            };
            if package {
                self.package = Some(check);
            }
            if database {
                self.database = Some(check);
            }
        }
        Ok(())
    }

    /// Fills the parts not set here from `base`.
    pub fn over(self, base: SigLevel) -> SigLevel {
        SigLevel {
            package: self.package.unwrap_or(base.package),
            database: self.database.unwrap_or(base.database),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(v: &str) -> Result<SigLevel, String> {
        let mut s = SigLevelSetting::default();
        s.apply(v)?;
        Ok(s.over(SigLevel::default()))
    }

    #[test]
    fn plain_and_prefixed_tokens() {
        let never = parse("Never").unwrap();
        assert_eq!((never.package, never.database), (Check::Never, Check::Never));
        let mixed = parse("Required DatabaseOptional TrustedOnly").unwrap();
        assert_eq!((mixed.package, mixed.database), (Check::Required, Check::Optional));
        let pkg = parse("PackageNever").unwrap();
        assert_eq!((pkg.package, pkg.database), (Check::Never, Check::Required));
    }

    #[test]
    fn repo_overrides_only_what_it_sets() {
        let mut global = SigLevelSetting::default();
        global.apply("Optional").unwrap();
        let mut repo = SigLevelSetting::default();
        repo.apply("PackageRequired").unwrap();
        let level = repo.over(global.over(SigLevel::default()));
        assert_eq!((level.package, level.database), (Check::Required, Check::Optional));
    }

    #[test]
    fn invalid_token() {
        assert_eq!(parse("Required Sometimes"), Err("Sometimes".into()));
    }
}
