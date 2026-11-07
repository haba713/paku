// SPDX-License-Identifier: MIT

//! The `desc`/`files` record format of pacman-style databases.
//!
//! ```text
//! %NAME%
//! foo
//!
//! %DEPENDS%
//! bar
//! baz>=1.0
//! ```
//!
//! A `%KEY%` header line is followed by one value per line; a blank line ends the
//! section. Keys keep their file order.

/// One parsed `desc` (or `files`) record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Desc {
    sections: Vec<(String, Vec<String>)>,
}

impl Desc {
    /// Parses a record. Lines before the first header are ignored.
    pub fn parse(text: &str) -> Self {
        let mut desc = Desc::default();
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if let Some(key) = header_key(line) {
                current = Some(desc.section_index(key));
                continue;
            }
            if line.is_empty() {
                current = None;
                continue;
            }
            if let Some(i) = current {
                desc.sections[i].1.push(line.to_string());
            }
        }
        desc
    }

    /// All values of `key` (empty if absent).
    pub fn get(&self, key: &str) -> &[String] {
        self.sections
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// First value of `key`, if any.
    pub fn first(&self, key: &str) -> Option<&str> {
        self.get(key).first().map(String::as_str)
    }

    /// Replaces the values of `key`, appending the section if new.
    pub fn set(&mut self, key: &str, values: Vec<String>) {
        let i = self.section_index(key);
        self.sections[i].1 = values;
    }

    /// Removes `key` entirely.
    pub fn remove(&mut self, key: &str) {
        self.sections.retain(|(k, _)| k != key);
    }

    /// Serializes back to the on-disk format; empty sections are omitted.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (key, values) in self.sections.iter().filter(|(_, v)| !v.is_empty()) {
            out.push('%');
            out.push_str(key);
            out.push_str("%\n");
            for v in values {
                out.push_str(v);
                out.push('\n');
            }
            out.push('\n');
        }
        out
    }

    fn section_index(&mut self, key: &str) -> usize {
        if let Some(i) = self.sections.iter().position(|(k, _)| k == key) {
            return i;
        }
        self.sections.push((key.to_string(), Vec::new()));
        self.sections.len() - 1
    }
}

fn header_key(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('%')?.strip_suffix('%')?;
    let valid = !inner.is_empty()
        && inner
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
    valid.then_some(inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "%FILENAME%\nfoo-1.0-1-any.pkg.tar.zst\n\n%NAME%\nfoo\n\n\
                          %DEPENDS%\nbar\nbaz>=2\n\n";

    #[test]
    fn parses_sections() {
        let d = Desc::parse(SAMPLE);
        assert_eq!(d.first("NAME"), Some("foo"));
        assert_eq!(d.get("DEPENDS"), ["bar", "baz>=2"]);
        assert!(d.get("MISSING").is_empty());
    }

    #[test]
    fn handles_crlf_and_missing_trailing_blank() {
        let d = Desc::parse("%NAME%\r\nfoo\r\n%VERSION%\r\n1-1");
        assert_eq!(d.first("NAME"), Some("foo"));
        assert_eq!(d.first("VERSION"), Some("1-1"));
    }

    #[test]
    fn round_trips() {
        let d = Desc::parse(SAMPLE);
        assert_eq!(Desc::parse(&d.to_text()), d);
        assert_eq!(d.to_text(), SAMPLE);
    }

    #[test]
    fn set_and_remove() {
        let mut d = Desc::parse(SAMPLE);
        d.set("REASON", vec!["1".into()]);
        d.remove("DEPENDS");
        assert_eq!(d.first("REASON"), Some("1"));
        assert!(d.get("DEPENDS").is_empty());
    }
}
