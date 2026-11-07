// SPDX-License-Identifier: GPL-2.0-or-later

//! Builders for package archives used in install tests.

use std::path::{Path, PathBuf};

use paku_platform::TempDir;

pub fn tempdir() -> TempDir {
    TempDir::new("paku-test-").unwrap()
}

enum Item {
    Dir(String),
    File(String, Vec<u8>),
    Symlink(String, String),
    Hardlink(String, String),
}

/// Builds a gzip-compressed package archive.
pub struct PkgBuilder {
    name: String,
    version: String,
    depends: Vec<String>,
    items: Vec<Item>,
    pkginfo: bool,
}

impl PkgBuilder {
    pub fn new(name: &str, version: &str) -> Self {
        PkgBuilder {
            name: name.into(),
            version: version.into(),
            depends: Vec::new(),
            items: Vec::new(),
            pkginfo: true,
        }
    }

    pub fn dir(mut self, path: &str) -> Self {
        self.items.push(Item::Dir(path.into()));
        self
    }

    pub fn file(mut self, path: &str, data: &[u8]) -> Self {
        self.items.push(Item::File(path.into(), data.to_vec()));
        self
    }

    pub fn symlink(mut self, path: &str, target: &str) -> Self {
        self.items.push(Item::Symlink(path.into(), target.into()));
        self
    }

    pub fn hardlink(mut self, path: &str, target: &str) -> Self {
        self.items.push(Item::Hardlink(path.into(), target.into()));
        self
    }

    pub fn without_pkginfo(mut self) -> Self {
        self.pkginfo = false;
        self
    }

    /// File name as it appears in a repository.
    pub fn filename(&self) -> String {
        format!("{}-{}-any.pkg.tar.gz", self.name, self.version)
    }

    /// Writes the archive into `dir` and returns its path.
    pub fn write(&self, dir: &Path) -> PathBuf {
        let mut b = tar::Builder::new(Vec::new());
        if self.pkginfo {
            append(
                &mut b,
                ".PKGINFO",
                tar::EntryType::Regular,
                self.pkginfo_text().as_bytes(),
                None,
            );
        }
        append(&mut b, ".MTREE", tar::EntryType::Regular, b"\x1f\x8bfake", None);
        for item in &self.items {
            match item {
                Item::Dir(p) => append(&mut b, p, tar::EntryType::Directory, b"", None),
                Item::File(p, d) => append(&mut b, p, tar::EntryType::Regular, d, None),
                Item::Symlink(p, t) => append(&mut b, p, tar::EntryType::Symlink, b"", Some(t)),
                Item::Hardlink(p, t) => append(&mut b, p, tar::EntryType::Link, b"", Some(t)),
            }
        }
        let tar = b.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &tar).unwrap();
        let path = dir.join(self.filename());
        std::fs::write(&path, gz.finish().unwrap()).unwrap();
        path
    }

    fn pkginfo_text(&self) -> String {
        let mut s = format!(
            "pkgname = {}\npkgver = {}\nsize = 10\narch = any\n",
            self.name, self.version
        );
        for d in &self.depends {
            s.push_str(&format!("depend = {d}\n"));
        }
        s
    }
}

/// Appends an entry with the name written raw, so tests can include unsafe paths
/// (`../x`) that `tar::Header::set_path` would refuse.
fn append(b: &mut tar::Builder<Vec<u8>>, path: &str, ty: tar::EntryType, data: &[u8], link: Option<&str>) {
    let mut h = tar::Header::new_old();
    let name = &mut h.as_old_mut().name;
    name[..path.len()].copy_from_slice(path.as_bytes());
    if let Some(l) = link {
        let ln = &mut h.as_old_mut().linkname;
        ln[..l.len()].copy_from_slice(l.as_bytes());
    }
    h.set_entry_type(ty);
    h.set_size(data.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    b.append(&h, data).unwrap();
}
