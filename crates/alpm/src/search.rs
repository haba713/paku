// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! Package search (role of libalpm's `_alpm_filter_by_search`).

use paku_formats::depend::Depend;
use paku_formats::meta::PkgMeta;
use regex::{Regex, RegexBuilder};

use crate::error::{Error, Result};

/// Compiled search terms; a package matches when every term matches.
#[derive(Debug)]
pub struct Search {
    terms: Vec<Regex>,
}

impl Search {
    /// Compiles case-insensitive regular expressions for each term.
    pub fn new(terms: &[String]) -> Result<Self> {
        let terms = terms
            .iter()
            .map(|t| {
                RegexBuilder::new(t)
                    .case_insensitive(true)
                    .build()
                    .map_err(|_| Error::BadRegex(t.clone()))
            })
            .collect::<Result<_>>()?;
        Ok(Self { terms })
    }

    /// Whether `pkg` matches all terms by name, description or provided names.
    /// No terms matches everything.
    pub fn matches(&self, pkg: &PkgMeta) -> bool {
        self.terms.iter().all(|re| term_matches(re, pkg))
    }
}

fn term_matches(re: &Regex, pkg: &PkgMeta) -> bool {
    re.is_match(&pkg.name) || re.is_match(&pkg.desc) || pkg.provides.iter().any(|p| re.is_match(&Depend::parse(p).name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, desc: &str, provides: &[&str]) -> PkgMeta {
        PkgMeta {
            name: name.into(),
            desc: desc.into(),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            ..PkgMeta::default()
        }
    }

    #[test]
    fn all_terms_must_match() {
        let s = Search::new(&["rip".into(), "GREP".into()]).unwrap();
        assert!(s.matches(&pkg("ripgrep", "fast grep", &[])));
        assert!(!s.matches(&pkg("ripper", "audio", &[])));
    }

    #[test]
    fn matches_provides_and_regex() {
        let s = Search::new(&["^cc$".into()]).unwrap();
        assert!(s.matches(&pkg("gcc", "compiler", &["cc=15"])));
        assert!(!s.matches(&pkg("gcc", "compiler", &[])));
    }

    #[test]
    fn bad_regex_is_error() {
        assert!(Search::new(&["(".into()]).is_err());
    }
}
