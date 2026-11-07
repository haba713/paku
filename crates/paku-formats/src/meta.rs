// SPDX-License-Identifier: MIT

//! Package metadata as stored in sync database `desc` records and `.PKGINFO`.

use crate::desc::Desc;
use crate::pkginfo::PkgInfo;

/// Metadata of one package, independent of where it was read from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PkgMeta {
    /// Package file name in the repository (sync DB only).
    pub filename: Option<String>,
    /// Package name.
    pub name: String,
    /// Split-package base name.
    pub base: Option<String>,
    /// Full version, `[epoch~]pkgver-pkgrel`.
    pub version: String,
    /// One-line description.
    pub desc: String,
    /// Groups the package belongs to.
    pub groups: Vec<String>,
    /// Compressed (download) size in bytes.
    pub csize: u64,
    /// Installed size in bytes.
    pub isize: u64,
    /// Hex SHA-256 of the package file.
    pub sha256: Option<String>,
    /// Base64 detached PGP signature.
    pub pgpsig: Option<String>,
    /// Upstream URL.
    pub url: Option<String>,
    /// Licenses.
    pub licenses: Vec<String>,
    /// Architecture (`x86_64`, `aarch64`, `any`).
    pub arch: String,
    /// Build date, seconds since the Unix epoch.
    pub builddate: i64,
    /// Packager name and email.
    pub packager: Option<String>,
    /// Runtime dependencies.
    pub depends: Vec<String>,
    /// Optional dependencies (`name: reason`).
    pub optdepends: Vec<String>,
    /// Build-time dependencies.
    pub makedepends: Vec<String>,
    /// Test dependencies.
    pub checkdepends: Vec<String>,
    /// Virtual packages provided.
    pub provides: Vec<String>,
    /// Conflicting packages.
    pub conflicts: Vec<String>,
    /// Packages this one replaces.
    pub replaces: Vec<String>,
}

impl PkgMeta {
    /// Builds metadata from a sync database `desc` record.
    /// Returns `None` when `NAME` or `VERSION` is missing.
    pub fn from_desc(d: &Desc) -> Option<Self> {
        let s = |k: &str| d.first(k).map(str::to_string);
        let v = |k: &str| d.get(k).to_vec();
        let n = |k: &str| d.first(k).and_then(|x| x.parse().ok()).unwrap_or(0);
        Some(PkgMeta {
            filename: s("FILENAME"),
            name: s("NAME")?,
            base: s("BASE"),
            version: s("VERSION")?,
            desc: s("DESC").unwrap_or_default(),
            groups: v("GROUPS"),
            csize: n("CSIZE") as u64,
            isize: d
                .first("ISIZE")
                .or(d.first("SIZE"))
                .and_then(|x| x.parse().ok())
                .unwrap_or(0),
            sha256: s("SHA256SUM"),
            pgpsig: s("PGPSIG"),
            url: s("URL"),
            licenses: v("LICENSE"),
            arch: s("ARCH").unwrap_or_default(),
            builddate: n("BUILDDATE"),
            packager: s("PACKAGER"),
            depends: v("DEPENDS"),
            optdepends: v("OPTDEPENDS"),
            makedepends: v("MAKEDEPENDS"),
            checkdepends: v("CHECKDEPENDS"),
            provides: v("PROVIDES"),
            conflicts: v("CONFLICTS"),
            replaces: v("REPLACES"),
        })
    }

    /// Builds metadata from a package's `.PKGINFO`.
    /// Returns `None` when `pkgname` or `pkgver` is missing.
    pub fn from_pkginfo(p: &PkgInfo) -> Option<Self> {
        let s = |k: &str| p.get(k).map(str::to_string);
        let v = |k: &str| p.get_all(k).into_iter().map(str::to_string).collect();
        Some(PkgMeta {
            filename: None,
            name: s("pkgname")?,
            base: s("pkgbase"),
            version: s("pkgver")?,
            desc: s("pkgdesc").unwrap_or_default(),
            groups: v("group"),
            csize: 0,
            isize: p.get("size").and_then(|x| x.parse().ok()).unwrap_or(0),
            sha256: None,
            pgpsig: None,
            url: s("url"),
            licenses: v("license"),
            arch: s("arch").unwrap_or_default(),
            builddate: p.get("builddate").and_then(|x| x.parse().ok()).unwrap_or(0),
            packager: s("packager"),
            depends: v("depend"),
            optdepends: v("optdepend"),
            makedepends: v("makedepend"),
            checkdepends: v("checkdepend"),
            provides: v("provides"),
            conflicts: v("conflict"),
            replaces: v("replaces"),
        })
    }

    /// Writes the fields shared by sync and local `desc` records.
    /// Local-only fields (`INSTALLDATE`, `REASON`, ...) are added by the caller.
    pub fn to_local_desc(&self) -> Desc {
        let mut d = Desc::default();
        let one = |x: &str| vec![x.to_string()];
        d.set("NAME", one(&self.name));
        d.set("VERSION", one(&self.version));
        d.set("BASE", self.base.iter().cloned().collect());
        d.set("DESC", one(&self.desc));
        d.set("URL", self.url.iter().cloned().collect());
        d.set("ARCH", one(&self.arch));
        d.set("BUILDDATE", one(&self.builddate.to_string()));
        d.set("PACKAGER", self.packager.iter().cloned().collect());
        d.set("SIZE", one(&self.isize.to_string()));
        d.set("GROUPS", self.groups.clone());
        d.set("LICENSE", self.licenses.clone());
        d.set("REPLACES", self.replaces.clone());
        d.set("DEPENDS", self.depends.clone());
        d.set("OPTDEPENDS", self.optdepends.clone());
        d.set("CONFLICTS", self.conflicts.clone());
        d.set("PROVIDES", self.provides.clone());
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_sync_desc() {
        let d = Desc::parse(
            "%FILENAME%\nfoo-1-1-any.pkg.tar.zst\n\n%NAME%\nfoo\n\n%VERSION%\n1-1\n\n\
             %CSIZE%\n10\n\n%ISIZE%\n20\n\n%DEPENDS%\nbar\n\n",
        );
        let m = PkgMeta::from_desc(&d).unwrap();
        assert_eq!(m.name, "foo");
        assert_eq!(m.csize, 10);
        assert_eq!(m.isize, 20);
        assert_eq!(m.depends, ["bar"]);
    }

    #[test]
    fn requires_name_and_version() {
        assert!(PkgMeta::from_desc(&Desc::parse("%NAME%\nfoo\n")).is_none());
    }

    #[test]
    fn from_pkginfo_and_back_to_local() {
        let p = PkgInfo::parse("pkgname = foo\npkgver = 2-1\nsize = 5\ndepend = bar\n");
        let m = PkgMeta::from_pkginfo(&p).unwrap();
        let local = PkgMeta::from_desc(&m.to_local_desc()).unwrap();
        assert_eq!(local.name, "foo");
        assert_eq!(local.isize, 5);
        assert_eq!(local.depends, ["bar"]);
    }
}
