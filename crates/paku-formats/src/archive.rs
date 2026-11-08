// SPDX-License-Identifier: GPL-2.0-or-later

//! Compression detection and decompression for databases and package archives.
//!
//! Compression is detected from magic bytes, not file names: MSYS2 `.db` files are
//! zstd-compressed tarballs despite the extension.

use std::io::{BufRead, BufReader, Cursor, Read};

use crate::{FormatError, Result};

/// Compression formats found in MSYS2 repositories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Zstandard (`.zst`), the current default.
    Zstd,
    /// gzip (`.gz`), used by `.MTREE` and old databases.
    Gzip,
    /// xz (`.xz`), used by older packages.
    Xz,
    /// No compression.
    None,
}

const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const GZIP_MAGIC: &[u8] = &[0x1F, 0x8B];
const XZ_MAGIC: &[u8] = &[0xFD, b'7', b'z', b'X', b'Z', 0x00];
const BZIP2_MAGIC: &[u8] = b"BZh";

/// Detects compression from the leading bytes of a stream.
pub fn detect(head: &[u8]) -> Result<Compression> {
    if head.starts_with(ZSTD_MAGIC) {
        return Ok(Compression::Zstd);
    }
    if head.starts_with(GZIP_MAGIC) {
        return Ok(Compression::Gzip);
    }
    if head.starts_with(XZ_MAGIC) {
        return Ok(Compression::Xz);
    }
    if head.starts_with(BZIP2_MAGIC) {
        return Err(FormatError::Invalid("bzip2 compression is not supported".into()));
    }
    Ok(Compression::None)
}

/// Wraps `input` in a decompressor chosen by its magic bytes.
pub fn decompress<'a, R: Read + 'a>(input: R) -> Result<Box<dyn Read + 'a>> {
    let mut buffered = BufReader::new(input);
    let compression = detect(buffered.fill_buf()?)?;
    let reader: Box<dyn Read + 'a> = match compression {
        Compression::Zstd => Box::new(
            ruzstd::decoding::StreamingDecoder::new(buffered)
                .map_err(|e| FormatError::Invalid(format!("zstd: {e}")))?,
        ),
        Compression::Gzip => Box::new(flate2::bufread::MultiGzDecoder::new(buffered)),
        Compression::Xz => Box::new(Cursor::new(decompress_xz(&mut buffered)?)),
        Compression::None => Box::new(buffered),
    };
    Ok(reader)
}

fn decompress_xz(input: &mut impl BufRead) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    lzma_rs::xz_decompress(input, &mut out).map_err(|e| FormatError::Invalid(format!("xz: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_magic() {
        assert_eq!(detect(ZSTD_MAGIC).unwrap(), Compression::Zstd);
        assert_eq!(detect(&[0x1F, 0x8B, 8]).unwrap(), Compression::Gzip);
        assert_eq!(detect(XZ_MAGIC).unwrap(), Compression::Xz);
        assert_eq!(detect(b"plain").unwrap(), Compression::None);
        assert!(detect(b"BZh9").is_err());
    }

    #[test]
    fn decompresses_gzip_and_plain() {
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(b"hello").unwrap();
        let data = gz.finish().unwrap();
        let mut out = String::new();
        decompress(&data[..]).unwrap().read_to_string(&mut out).unwrap();
        assert_eq!(out, "hello");

        out.clear();
        decompress(&b"plain"[..]).unwrap().read_to_string(&mut out).unwrap();
        assert_eq!(out, "plain");
    }
}
