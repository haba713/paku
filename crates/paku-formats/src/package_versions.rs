// SPDX-License-Identifier: MIT

//! Git for Windows' `/etc/package-versions.txt`: one `name version` pair per line,
//! listing the MSYS2 packages bundled with Git.

/// Parses the file into `(name, version)` pairs, skipping blank or malformed lines.
/// Duplicate names keep their first occurrence.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(version), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if out.iter().any(|(n, _)| n == name) {
            continue;
        }
        out.push((name.to_string(), version.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pairs_and_dedups() {
        let text = "bash 5.3.015-2\r\ngrep 1~3.0-7\n\nbad line here\ngrep 9-9\n";
        let parsed = parse(text);
        assert_eq!(
            parsed,
            vec![
                ("bash".to_string(), "5.3.015-2".to_string()),
                ("grep".to_string(), "1~3.0-7".to_string()),
            ]
        );
    }
}
