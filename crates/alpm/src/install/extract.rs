// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Role of extract_single_file / commit_single_pkg in lib/libalpm/add.c.

//! Unpacking a package archive into a staging directory, relocated by [`Layout`].
//!
//! Windows symlinks need extra privileges, so symlinks and hard links are
//! materialized as copies of their target (like MSYS's default `winsymlinks:deepcopy`).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use paku_formats::meta::PkgMeta;
use paku_formats::pkgfile::{EntryKind, for_each_entry};
use paku_formats::pkginfo::PkgInfo;
use paku_layout::{Layout, Mapped};

use crate::error::{Error, Result, io_err};

/// What a package archive contained.
#[derive(Debug, Default)]
pub struct Extracted {
    /// Metadata from `.PKGINFO`.
    pub meta: PkgMeta,
    /// Installed paths relative to the prefix; directories end with `/`. Sorted.
    pub files: Vec<String>,
    /// Backup (configuration) files, relative to the prefix.
    pub backup: Vec<String>,
    /// Raw `.MTREE` (gzip-compressed), stored in the local database.
    pub mtree: Option<Vec<u8>>,
    /// Raw `.INSTALL` scriptlet, if any.
    pub install: Option<Vec<u8>>,
    /// Links whose target is not part of the package; not installed.
    pub skipped_links: Vec<String>,
}

#[derive(Default)]
struct Pending {
    pkginfo: Option<String>,
    links: Vec<(String, String)>,
}

/// Extracts `pkgfile` into `staging`, which mirrors the install prefix.
pub fn extract(pkgfile: &Path, layout: &Layout, staging: &Path) -> Result<Extracted> {
    let file = File::open(pkgfile).map_err(io_err(pkgfile))?;
    let mut out = Extracted::default();
    let mut pending = Pending::default();
    for_each_entry(io::BufReader::new(file), |path, kind, reader| {
        handle_entry(layout, staging, path, kind, reader, &mut out, &mut pending)
            .map_err(|e| paku_formats::FormatError::Invalid(e.to_string()))
    })?;
    let name = pkgfile
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let info = pending.pkginfo.as_deref().map(PkgInfo::parse);
    out.meta = info
        .as_ref()
        .and_then(PkgMeta::from_pkginfo)
        .ok_or(Error::BadPackage(name))?;
    out.backup = info.as_ref().map(|i| backup_paths(layout, i)).unwrap_or_default();
    resolve_links(staging, pending.links, &mut out)?;
    out.files.sort();
    out.files.dedup();
    Ok(out)
}

/// `backup = <archive path>` entries of `.PKGINFO`, relocated like the files.
fn backup_paths(layout: &Layout, info: &PkgInfo) -> Vec<String> {
    info.get_all("backup")
        .into_iter()
        .filter_map(|path| match layout.map(path) {
            Ok(Mapped::Install(rel)) => Some(rel),
            _ => None,
        })
        .collect()
}

fn handle_entry(
    layout: &Layout,
    staging: &Path,
    path: &str,
    kind: EntryKind,
    reader: &mut dyn Read,
    out: &mut Extracted,
    pending: &mut Pending,
) -> Result<()> {
    let rel = match layout.map(path)? {
        Mapped::Root => return Ok(()),
        Mapped::Metadata(name) => return read_metadata(&name, reader, out, pending),
        Mapped::Install(rel) => rel,
    };
    let dest = layout_path(staging, &rel);
    match kind {
        EntryKind::Dir => {
            fs::create_dir_all(&dest).map_err(io_err(&dest))?;
            out.files.push(dir_entry(&rel));
        }
        EntryKind::File => {
            write_file(&dest, reader)?;
            out.files.push(rel);
        }
        EntryKind::Symlink(target) => {
            let target_rel = link_target_rel(layout, &rel, &target)?;
            pending.links.push((rel, target_rel));
        }
        EntryKind::Hardlink(target) => pending.links.push((rel, archive_rel(layout, &target)?)),
        EntryKind::Other => {}
    }
    Ok(())
}

fn read_metadata(name: &str, reader: &mut dyn Read, out: &mut Extracted, pending: &mut Pending) -> Result<()> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data).map_err(|e| io_err(name)(e))?;
    match name {
        ".PKGINFO" => pending.pkginfo = Some(String::from_utf8_lossy(&data).into_owned()),
        ".MTREE" => out.mtree = Some(data),
        ".INSTALL" => out.install = Some(data),
        _ => {}
    }
    Ok(())
}

fn write_file(dest: &Path, reader: &mut dyn Read) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    let mut file = File::create(dest).map_err(io_err(dest))?;
    io::copy(reader, &mut file).map_err(io_err(dest))?;
    Ok(())
}

/// Copies link targets (in dependency order, so links to links work).
fn resolve_links(staging: &Path, mut links: Vec<(String, String)>, out: &mut Extracted) -> Result<()> {
    loop {
        let before = links.len();
        let mut rest = Vec::new();
        for (link, target) in links {
            let src = layout_path(staging, &target);
            if !src.is_file() {
                rest.push((link, target));
                continue;
            }
            let dest = layout_path(staging, &link);
            write_file(&dest, &mut File::open(&src).map_err(io_err(&src))?)?;
            out.files.push(link);
        }
        links = rest;
        if links.is_empty() || links.len() == before {
            break;
        }
    }
    out.skipped_links.extend(links.into_iter().map(|(link, _)| link));
    Ok(())
}

/// A symlink target, relative to the link's directory (or absolute), as a
/// prefix-relative path.
fn link_target_rel(layout: &Layout, link_rel: &str, target: &str) -> Result<String> {
    if let Some(abs) = target.strip_prefix('/') {
        return archive_rel(layout, abs);
    }
    let mut parts: Vec<&str> = link_rel.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    Ok(parts.join("/"))
}

/// Relocates an archive path (hard link targets, absolute symlinks).
fn archive_rel(layout: &Layout, archive_path: &str) -> Result<String> {
    match layout.map(archive_path)? {
        Mapped::Install(rel) => Ok(rel),
        _ => Ok(String::new()),
    }
}

fn dir_entry(rel: &str) -> String {
    if rel.ends_with('/') {
        rel.to_string()
    } else {
        format!("{rel}/")
    }
}

fn layout_path(root: &Path, rel: &str) -> std::path::PathBuf {
    let mut p = root.to_path_buf();
    p.extend(rel.split('/').filter(|s| !s.is_empty()));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::testutil::{PkgBuilder, tempdir};

    #[test]
    fn extracts_relocated_files_metadata_and_links() {
        let dir = tempdir();
        let pkg = PkgBuilder::new("rsync", "3.5.2-1")
            .dir("usr/")
            .file("usr/bin/rsync.exe", b"MZ")
            .symlink("usr/bin/rsync-ssl", "rsync.exe")
            .symlink("usr/bin/dangling", "nothing.exe")
            .hardlink("usr/bin/rsync2.exe", "usr/bin/rsync.exe")
            .write(dir.path());
        let staging = dir.path().join("staging");
        let layout = Layout::new(dir.path().join("prefix"), "ucrt64");

        let out = extract(&pkg, &layout, &staging).unwrap();
        assert_eq!(out.meta.name, "rsync");
        assert_eq!(out.files, ["bin/rsync-ssl", "bin/rsync.exe", "bin/rsync2.exe"]);
        assert_eq!(fs::read(staging.join("bin/rsync-ssl")).unwrap(), b"MZ");
        assert_eq!(out.skipped_links, ["bin/dangling"]);
        assert!(out.mtree.is_some());
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempdir();
        let pkg = PkgBuilder::new("evil", "1-1")
            .file("../outside.txt", b"x")
            .write(dir.path());
        let layout = Layout::new(dir.path().join("prefix"), "ucrt64");
        assert!(extract(&pkg, &layout, &dir.path().join("staging")).is_err());
        assert!(!dir.path().join("outside.txt").exists());
    }

    #[test]
    fn missing_pkginfo_is_an_error() {
        let dir = tempdir();
        let pkg = PkgBuilder::new("x", "1-1").without_pkginfo().write(dir.path());
        let layout = Layout::new(dir.path().join("prefix"), "ucrt64");
        assert!(matches!(
            extract(&pkg, &layout, &dir.path().join("s")),
            Err(Error::BadPackage(_))
        ));
    }
}
