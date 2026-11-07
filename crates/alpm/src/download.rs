// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! File downloads with mirror fallback (role of libalpm's `dload.c`).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use paku_platform::http::Client;

use crate::error::{Error, Result, io_err};

/// Download settings.
#[derive(Debug, Clone)]
pub struct Options {
    /// HTTP `User-Agent` header.
    pub user_agent: String,
    /// Limit for connecting and for each wait for data; `None` disables it
    /// (`--disable-download-timeout`).
    pub timeout: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            user_agent: format!("paku/{}", env!("CARGO_PKG_VERSION")),
            timeout: Some(Duration::from_secs(30)),
        }
    }
}

/// Outcome of a conditional download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetched {
    /// A new copy was written.
    Updated,
    /// The local copy is already current; nothing was written.
    UpToDate,
}

/// Downloads `file` from the first server that has it into `dest`.
///
/// Unless `force` is set, an existing `dest` is revalidated with
/// `If-Modified-Since`, so unchanged files are not transferred again.
pub fn fetch(servers: &[String], file: &str, dest: &Path, force: bool, opts: &Options) -> Result<Fetched> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    let local_mtime = if force { None } else { mtime(dest) };
    let mut last_error = String::from("no servers");
    for server in servers {
        let url = format!("{}/{}", server.trim_end_matches('/'), file);
        match fetch_one(&url, dest, local_mtime, opts) {
            Ok(outcome) => return Ok(outcome),
            Err(e) => last_error = e,
        }
    }
    Err(Error::Download {
        file: file.to_string(),
        reason: last_error,
    })
}

fn fetch_one(
    url: &str,
    dest: &Path,
    local_mtime: Option<SystemTime>,
    opts: &Options,
) -> std::result::Result<Fetched, String> {
    if let Some(path) = file_url_path(url) {
        return fetch_file(&path, dest, local_mtime);
    }
    let client = Client::new(&opts.user_agent, opts.timeout).map_err(|e| format!("{url}: {e}"))?;
    let since = local_mtime.map(httpdate::fmt_http_date);
    let headers: Vec<(&str, &str)> = since.iter().map(|t| ("If-Modified-Since", t.as_str())).collect();
    let response = client.get(url, &headers).map_err(|e| format!("{url}: {e}"))?;
    match response.status {
        304 => return Ok(Fetched::UpToDate),
        200 => {}
        code => return Err(format!("{url}: HTTP {code}")),
    }
    let modified = response
        .last_modified
        .as_deref()
        .and_then(|v| httpdate::parse_http_date(v).ok());
    let reader = response;
    write_atomically(reader, dest, modified).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(Fetched::Updated)
}

fn fetch_file(src: &Path, dest: &Path, local_mtime: Option<SystemTime>) -> std::result::Result<Fetched, String> {
    let src_mtime = mtime(src);
    if let (Some(local), Some(remote)) = (local_mtime, src_mtime) {
        if remote <= local {
            return Ok(Fetched::UpToDate);
        }
    }
    let file = File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    write_atomically(file, dest, src_mtime).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(Fetched::Updated)
}

/// Writes to `<dest>.part` and renames, so a failed transfer never leaves a
/// truncated file in place. Stamps the server's modification time when known.
fn write_atomically(mut reader: impl Read, dest: &Path, modified: Option<SystemTime>) -> io::Result<()> {
    let part = part_path(dest);
    let mut out = File::create(&part)?;
    let copied = io::copy(&mut reader, &mut out).and_then(|_| out.sync_all());
    if let Err(e) = copied {
        drop(out);
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    if let Some(t) = modified {
        out.set_modified(t)?;
    }
    drop(out);
    fs::rename(&part, dest)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Local path of a `file://` URL (`file:///C:/x` and `file://C:/x` both work).
fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let has_drive_after_slash = bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':';
    let path = if has_drive_after_slash { &rest[1..] } else { rest };
    Some(PathBuf::from(path))
}

/// Result of [`fetch_package`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedPackage {
    /// Path of the verified package file in the cache.
    pub path: PathBuf,
    /// `true` when a valid copy was already cached and nothing was downloaded.
    pub was_cached: bool,
}

/// Makes sure package `filename` is in `cache_dir` with the expected SHA-256,
/// downloading it from the first working server if needed. A cached file with the
/// wrong checksum is replaced; a downloaded file with the wrong checksum is deleted.
pub fn fetch_package(
    servers: &[String],
    filename: &str,
    cache_dir: &Path,
    sha256: Option<&str>,
    opts: &Options,
) -> Result<CachedPackage> {
    let path = cache_dir.join(filename);
    if path.is_file() && checksum_ok(&path, sha256)? {
        return Ok(CachedPackage { path, was_cached: true });
    }
    fetch(servers, filename, &path, true, opts)?;
    if !checksum_ok(&path, sha256)? {
        let _ = fs::remove_file(&path);
        return Err(Error::ChecksumMismatch(filename.to_string()));
    }
    Ok(CachedPackage {
        path,
        was_cached: false,
    })
}

fn checksum_ok(path: &Path, expected: Option<&str>) -> Result<bool> {
    let Some(expected) = expected else {
        return Ok(true);
    };
    Ok(crate::checksum::sha256_file(path)?.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_server(dir: &Path) -> String {
        format!("file://{}", dir.display())
    }

    #[test]
    fn file_url_paths() {
        assert_eq!(file_url_path("file:///C:/x/y"), Some(PathBuf::from("C:/x/y")));
        assert_eq!(file_url_path("file:///tmp/y"), Some(PathBuf::from("/tmp/y")));
        assert_eq!(file_url_path("https://x"), None);
    }

    #[test]
    fn fetches_and_revalidates_from_file_server() {
        let repo = paku_platform::TempDir::new("paku-test-").unwrap();
        let out = paku_platform::TempDir::new("paku-test-").unwrap();
        fs::write(repo.path().join("r.db"), b"data").unwrap();
        let servers = vec!["file:///nonexistent-mirror".to_string(), file_server(repo.path())];
        let dest = out.path().join("sync/r.db");
        let opts = Options::default();

        assert_eq!(fetch(&servers, "r.db", &dest, false, &opts).unwrap(), Fetched::Updated);
        assert_eq!(fs::read(&dest).unwrap(), b"data");
        assert_eq!(fetch(&servers, "r.db", &dest, false, &opts).unwrap(), Fetched::UpToDate);
        assert_eq!(fetch(&servers, "r.db", &dest, true, &opts).unwrap(), Fetched::Updated);
    }

    #[test]
    fn reports_failure_from_all_mirrors() {
        let out = paku_platform::TempDir::new("paku-test-").unwrap();
        let servers = vec!["file:///nonexistent-mirror".to_string()];
        let err = fetch(&servers, "r.db", &out.path().join("r.db"), false, &Options::default());
        assert!(matches!(err, Err(Error::Download { .. })));
    }

    #[test]
    fn package_cache_and_checksums() {
        let repo = paku_platform::TempDir::new("paku-test-").unwrap();
        let cache = paku_platform::TempDir::new("paku-test-").unwrap();
        fs::write(repo.path().join("p.pkg.tar"), b"abc").unwrap();
        let servers = vec![file_server(repo.path())];
        let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let opts = Options::default();

        let first = fetch_package(&servers, "p.pkg.tar", cache.path(), Some(good), &opts).unwrap();
        assert!(!first.was_cached);
        let second = fetch_package(&servers, "p.pkg.tar", cache.path(), Some(good), &opts).unwrap();
        assert!(second.was_cached);

        let bad = fetch_package(&servers, "p.pkg.tar", cache.path(), Some("00"), &opts);
        assert!(matches!(bad, Err(Error::ChecksumMismatch(_))));
        assert!(!cache.path().join("p.pkg.tar").exists());
    }
}
