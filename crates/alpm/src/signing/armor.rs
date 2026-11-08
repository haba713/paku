// SPDX-License-Identifier: GPL-2.0-or-later

//! Base64 (RFC 4648) and OpenPGP ASCII armor (RFC 4880 §6) decoding.

use super::SigError;

/// Decodes standard base64, ignoring whitespace. Padding is optional.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, SigError> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | u32::from(sextet(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn sextet(c: u8) -> Result<u8, SigError> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(SigError::Malformed),
    }
}

#[derive(PartialEq)]
enum Part {
    Outside,
    Headers,
    Body,
}

/// Returns binary OpenPGP data: armored input is decoded (all blocks, concatenated),
/// binary input is returned as is.
pub fn dearmor(data: &[u8]) -> Result<Vec<u8>, SigError> {
    let Some(text) = armored_text(data) else {
        return Ok(data.to_vec());
    };
    let mut out = Vec::new();
    let mut body = String::new();
    let mut part = Part::Outside;
    for line in text.lines().map(str::trim_end) {
        if line.starts_with("-----BEGIN PGP") {
            part = Part::Headers;
            continue;
        }
        if line.starts_with("-----END PGP") {
            out.extend(base64_decode(&body)?);
            body.clear();
            part = Part::Outside;
            continue;
        }
        // Armor headers (`Comment: ...`) end at the first blank line.
        if part == Part::Headers && (line.is_empty() || line.contains(": ")) {
            part = if line.is_empty() { Part::Body } else { Part::Headers };
            continue;
        }
        if part == Part::Outside {
            continue;
        }
        part = Part::Body;
        // A line starting with `=` is the CRC-24 checksum, not data.
        if !line.starts_with('=') {
            body.push_str(line);
        }
    }
    Ok(out)
}

fn armored_text(data: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(data).ok()?;
    text.trim_start().starts_with("-----BEGIN PGP").then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert!(base64_decode("Zm9*").is_err());
    }

    #[test]
    fn armored_and_binary_keys_match() {
        let asc = include_bytes!("../../testdata/signing/test-key.asc");
        let bin = include_bytes!("../../testdata/signing/test-key.gpg");
        assert_eq!(dearmor(asc).unwrap(), bin.to_vec());
        assert_eq!(dearmor(bin).unwrap(), bin.to_vec());
    }
}
