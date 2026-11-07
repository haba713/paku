// SPDX-License-Identifier: MIT

//! Sync database archives (`<repo>.db`, `<repo>.files`).
//!
//! A sync database is a (usually zstd-compressed) tarball with one directory per
//! package, `<name>-<version>/`, holding a `desc` record and, in `.files`
//! databases, a `files` record.

use std::collections::HashMap;
use std::io::Read;

use crate::archive::decompress;
use crate::desc::Desc;
use crate::{FormatError, Result};

/// One package directory from a sync database.
#[derive(Debug, Clone, Default)]
pub struct SyncEntry {
    /// Directory name, `<name>-<version>`.
    pub dir: String,
    /// The `desc` record.
    pub desc: Desc,
    /// The `files` record, present in `.files` databases.
    pub files: Option<Desc>,
}

/// Reads all package entries from a (compressed) sync database stream.
/// Entries without a `desc` record are dropped.
pub fn read<R: Read>(input: R) -> Result<Vec<SyncEntry>> {
    let mut archive = tar::Archive::new(decompress(input)?);
    let mut entries: Vec<SyncEntry> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().replace('\\', "/");
        let Some((dir, file)) = path.split_once('/') else {
            continue;
        };
        if file != "desc" && file != "files" {
            continue;
        }
        let mut text = String::new();
        entry
            .read_to_string(&mut text)
            .map_err(|e| FormatError::Invalid(format!("{path}: {e}")))?;
        let i = *index.entry(dir.to_string()).or_insert_with(|| {
            entries.push(SyncEntry {
                dir: dir.to_string(),
                ..SyncEntry::default()
            });
            entries.len() - 1
        });
        let record = Desc::parse(&text);
        match file {
            "desc" => entries[i].desc = record,
            _ => entries[i].files = Some(record),
        }
    }
    entries.retain(|e| e.desc.first("NAME").is_some());
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, data.as_bytes()).unwrap();
    }

    #[test]
    fn reads_desc_and_files() {
        let mut b = tar::Builder::new(Vec::new());
        add(&mut b, "foo-1-1/desc", "%NAME%\nfoo\n\n%VERSION%\n1-1\n\n");
        add(&mut b, "foo-1-1/files", "%FILES%\nucrt64/\nucrt64/bin/foo.exe\n\n");
        add(&mut b, "orphan-1-1/files", "%FILES%\nx\n\n");
        let data = b.into_inner().unwrap();

        let entries = read(&data[..]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].dir, "foo-1-1");
        assert_eq!(entries[0].desc.first("NAME"), Some("foo"));
        let files = entries[0].files.as_ref().unwrap();
        assert_eq!(files.get("FILES"), ["ucrt64/", "ucrt64/bin/foo.exe"]);
    }
}
