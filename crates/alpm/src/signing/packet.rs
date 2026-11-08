// SPDX-License-Identifier: GPL-2.0-or-later

//! OpenPGP packet framing (RFC 4880 §4) and a bounds-checked body reader.

use super::SigError;

pub const TAG_SIGNATURE: u8 = 2;
pub const TAG_PUBLIC_KEY: u8 = 6;
pub const TAG_USER_ID: u8 = 13;
pub const TAG_PUBLIC_SUBKEY: u8 = 14;

/// One packet: its tag and body.
#[derive(Debug, Clone, Copy)]
pub struct Packet<'a> {
    pub tag: u8,
    pub body: &'a [u8],
}

/// Splits `data` into packets. Partial body lengths (only used for streamed
/// data packets, never in keys or signatures) are rejected.
pub fn packets(data: &[u8]) -> Result<Vec<Packet<'_>>, SigError> {
    let mut out = Vec::new();
    let mut r = Reader::new(data);
    while !r.is_empty() {
        let first = r.u8()?;
        if first & 0x80 == 0 {
            return Err(SigError::Malformed);
        }
        let (tag, len) = if first & 0x40 != 0 {
            (first & 0x3f, new_length(&mut r)?)
        } else {
            ((first >> 2) & 0x0f, old_length(&mut r, first & 0x03)?)
        };
        out.push(Packet {
            tag,
            body: r.take(len)?,
        });
    }
    Ok(out)
}

fn new_length(r: &mut Reader) -> Result<usize, SigError> {
    let o1 = usize::from(r.u8()?);
    match o1 {
        0..=191 => Ok(o1),
        192..=223 => Ok(((o1 - 192) << 8) + usize::from(r.u8()?) + 192),
        255 => Ok(r.u32()? as usize),
        _ => Err(SigError::Unsupported("partial body length".into())),
    }
}

fn old_length(r: &mut Reader, kind: u8) -> Result<usize, SigError> {
    match kind {
        0 => Ok(usize::from(r.u8()?)),
        1 => Ok(usize::from(r.u16()?)),
        2 => Ok(r.u32()? as usize),
        _ => Ok(r.remaining()),
    }
}

/// Reads big-endian fields from a byte slice, failing instead of panicking.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], SigError> {
        let end = self.pos.checked_add(n).ok_or(SigError::Malformed)?;
        let slice = self.data.get(self.pos..end).ok_or(SigError::Malformed)?;
        self.pos = end;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, SigError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, SigError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, SigError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A multiprecision integer: 16-bit bit count, then the big-endian bytes.
    pub fn mpi(&mut self) -> Result<&'a [u8], SigError> {
        let bits = usize::from(self.u16()?);
        self.take(bits.div_ceil(8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_old_format_headers() {
        // new format, tag 2, one-octet length 3
        let data = [
            0xC2, 3, 1, 2, 3, /* old format, tag 13, 1-byte length */ 0xB4, 1, b'x',
        ];
        let p = packets(&data).unwrap();
        assert_eq!((p[0].tag, p[0].body), (2, &[1u8, 2, 3][..]));
        assert_eq!((p[1].tag, p[1].body), (13, &b"x"[..]));
    }

    #[test]
    fn truncated_input_is_an_error() {
        assert!(packets(&[0xC2, 5, 1]).is_err());
        assert!(packets(&[0x02]).is_err());
        assert!(Reader::new(&[0, 16, 1]).mpi().is_err());
    }
}
