// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/common/ini.c.

//! INI reader for `pacman.conf`-style files, including `Include` directives.

use std::fs;
use std::path::Path;

/// One `key[ = value]` line with its section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Section name without brackets.
    pub section: String,
    /// Key name.
    pub key: String,
    /// Value, `None` for bare keys like `Color`.
    pub value: Option<String>,
    /// File and line, for error messages.
    pub origin: String,
}

const MAX_INCLUDE_DEPTH: usize = 10;

/// Reads `path`, following `Include = file` lines (within the current section).
pub fn read(path: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    let mut section = String::new();
    read_into(path, &mut section, &mut out, 0)?;
    Ok(out)
}

fn read_into(path: &Path, section: &mut String, out: &mut Vec<Entry>, depth: usize) -> Result<(), String> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(format!("{}: include depth exceeded", path.display()));
    }
    let text =
        fs::read_to_string(path).map_err(|e| format!("config file {} could not be read: {e}", path.display()))?;
    for (n, line) in text.lines().enumerate() {
        let origin = format!("{}:{}", path.display(), n + 1);
        let Some(entry) = parse_line(line, section, &origin)? else {
            continue;
        };
        if entry.key == "Include" {
            let target = entry.value.unwrap_or_default();
            read_into(&resolve(path, &target), section, out, depth + 1)?;
            continue;
        }
        out.push(entry);
    }
    Ok(())
}

/// Parses one line; updates `section` on headers. Returns `None` for
/// blank lines, comments and headers.
fn parse_line(line: &str, section: &mut String, origin: &str) -> Result<Option<Entry>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
        if name.is_empty() {
            return Err(format!("{origin}: bad section name"));
        }
        *section = name.to_string();
        return Ok(None);
    }
    if section.is_empty() {
        return Err(format!("{origin}: all directives must belong to a section"));
    }
    let (key, value) = match line.split_once('=') {
        Some((k, v)) => (k.trim(), Some(v.trim().to_string())),
        None => (line, None),
    };
    if key.is_empty() || value.as_deref() == Some("") {
        return Err(format!("{origin}: syntax error in config file- missing key or value"));
    }
    Ok(Some(Entry {
        section: section.clone(),
        key: key.to_string(),
        value,
        origin: origin.to_string(),
    }))
}

/// Relative includes resolve against the including file's directory.
fn resolve(from: &Path, target: &str) -> std::path::PathBuf {
    let target = Path::new(target);
    if target.is_absolute() {
        return target.to_path_buf();
    }
    from.parent().unwrap_or(Path::new(".")).join(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_keys_and_includes() {
        let dir = paku_platform::TempDir::new("paku-test-").unwrap();
        fs::write(dir.path().join("mirrors"), "Server = https://a/$repo\n").unwrap();
        let conf = dir.path().join("paku.conf");
        fs::write(
            &conf,
            "# comment\n[options]\nColor\nIgnorePkg = a b\n\n[ucrt64]\nInclude = mirrors\nServer = https://b\n",
        )
        .unwrap();

        let entries = read(&conf).unwrap();
        let simple: Vec<_> = entries
            .iter()
            .map(|e| (e.section.as_str(), e.key.as_str(), e.value.as_deref()))
            .collect();
        assert_eq!(
            simple,
            [
                ("options", "Color", None),
                ("options", "IgnorePkg", Some("a b")),
                ("ucrt64", "Server", Some("https://a/$repo")),
                ("ucrt64", "Server", Some("https://b")),
            ]
        );
    }

    #[test]
    fn errors() {
        let mut s = String::new();
        assert!(parse_line("Key = v", &mut s, "x").is_err());
        s = "options".into();
        assert!(parse_line("Key =", &mut s, "x").is_err());
        assert!(parse_line("[]", &mut s, "x").is_err());
    }
}
