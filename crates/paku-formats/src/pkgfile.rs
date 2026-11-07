// SPDX-License-Identifier: MIT

//! Package archives (`*.pkg.tar.zst`, `*.pkg.tar.xz`, ...): a compressed tarball whose
//! root holds metadata files (`.PKGINFO`, `.MTREE`, `.BUILDINFO`, optional `.INSTALL`)
//! next to the package's file tree.

use std::io::Read;

use crate::archive::decompress;
use crate::{FormatError, Result};

/// Kind of a tar entry, with link targets as stored in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link to the given target (usually relative to the link's directory).
    Symlink(String),
    /// Hard link to another archive path.
    Hardlink(String),
    /// Anything else (devices, FIFOs); never installed.
    Other,
}

/// Calls `visit(path, kind, contents)` for every entry of a (compressed) package archive.
/// Paths use `/` separators and are exactly as stored (not yet validated or relocated).
pub fn for_each_entry<R: Read>(
    input: R,
    mut visit: impl FnMut(&str, EntryKind, &mut dyn Read) -> Result<()>,
) -> Result<()> {
    let mut archive = tar::Archive::new(decompress(input)?);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = String::from_utf8_lossy(&entry.path_bytes()).replace('\\', "/");
        let kind = entry_kind(&entry)?;
        visit(&path, kind, &mut entry)?;
    }
    Ok(())
}

fn entry_kind<R: Read>(entry: &tar::Entry<'_, R>) -> Result<EntryKind> {
    let header = entry.header();
    let link = || -> Result<String> {
        let target = entry
            .link_name_bytes()
            .ok_or_else(|| FormatError::Invalid("link entry without target".into()))?;
        Ok(String::from_utf8_lossy(&target).replace('\\', "/"))
    };
    let ty = header.entry_type();
    Ok(if ty.is_file() || ty.is_contiguous() {
        EntryKind::File
    } else if ty.is_dir() {
        EntryKind::Dir
    } else if ty.is_symlink() {
        EntryKind::Symlink(link()?)
    } else if ty.is_hard_link() {
        EntryKind::Hardlink(link()?)
    } else {
        EntryKind::Other
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_files_dirs_and_links() {
        let mut b = tar::Builder::new(Vec::new());
        let mut add = |path: &str, ty: tar::EntryType, data: &[u8], link: Option<&str>| {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(ty);
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            if let Some(l) = link {
                h.set_link_name(l).unwrap();
            }
            h.set_cksum();
            b.append_data(&mut h, path, data).unwrap();
        };
        add(".PKGINFO", tar::EntryType::Regular, b"pkgname = x\n", None);
        add("usr/", tar::EntryType::Directory, b"", None);
        add("usr/bin/x.exe", tar::EntryType::Regular, b"MZ", None);
        add("usr/bin/y", tar::EntryType::Symlink, b"", Some("x.exe"));
        add("usr/bin/z.exe", tar::EntryType::Link, b"", Some("usr/bin/x.exe"));
        let data = b.into_inner().unwrap();

        let mut seen = Vec::new();
        for_each_entry(&data[..], |path, kind, r| {
            let mut s = String::new();
            r.read_to_string(&mut s).unwrap_or_default();
            seen.push((path.to_string(), kind, s));
            Ok(())
        })
        .unwrap();
        assert_eq!(seen[0], (".PKGINFO".into(), EntryKind::File, "pkgname = x\n".into()));
        assert_eq!(seen[1].1, EntryKind::Dir);
        assert_eq!(seen[2], ("usr/bin/x.exe".into(), EntryKind::File, "MZ".into()));
        assert_eq!(seen[3].1, EntryKind::Symlink("x.exe".into()));
        assert_eq!(seen[4].1, EntryKind::Hardlink("usr/bin/x.exe".into()));
    }
}
