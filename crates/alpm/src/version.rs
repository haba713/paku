// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of lib/libalpm/version.c (MSYS2 variant), partly adopted from rpm.

//! Package version comparison (`alpm_pkg_vercmp`).

use std::cmp::Ordering;

/// Compares two full versions `[epoch(~|:)]version[-release]`.
///
/// MSYS2 writes the epoch separator as `~` (`:` is invalid in Windows file names);
/// both are accepted. Releases are compared only when both sides have one.
pub fn vercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (e1, v1, r1) = parse_evr(a);
    let (e2, v2, r2) = parse_evr(b);
    rpmvercmp(e1, e2)
        .then_with(|| rpmvercmp(v1, v2))
        .then_with(|| match (r1, r2) {
            (Some(r1), Some(r2)) => rpmvercmp(r1, r2),
            _ => Ordering::Equal,
        })
}

/// Splits `evr` into epoch, version and optional release (port of `parseEVR`).
fn parse_evr(evr: &str) -> (&str, &str, Option<&str>) {
    let digits = evr.bytes().take_while(u8::is_ascii_digit).count();
    let after_epoch = &evr[digits..];
    let (epoch, rest) = match after_epoch.as_bytes().first() {
        Some(b'~' | b':') => {
            let epoch = &evr[..digits];
            (if epoch.is_empty() { "0" } else { epoch }, &after_epoch[1..])
        }
        _ => ("0", evr),
    };
    match rest.rfind('-') {
        Some(i) => (epoch, &rest[..i], Some(&rest[i + 1..])),
        None => (epoch, rest, None),
    }
}

/// Segment-wise comparison of alpha and numeric runs (port of `rpmvercmp`).
fn rpmvercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut one, mut two) = (0usize, 0usize);
    let (mut ptr1, mut ptr2) = (0usize, 0usize);

    while one < a.len() && two < b.len() {
        one += count_while(&a[one..], |c| !c.is_ascii_alphanumeric());
        two += count_while(&b[two..], |c| !c.is_ascii_alphanumeric());
        if one >= a.len() || two >= b.len() {
            break;
        }
        // differing separator lengths decide the comparison
        if one - ptr1 != two - ptr2 {
            return (one - ptr1).cmp(&(two - ptr2));
        }
        let isnum = a[one].is_ascii_digit();
        let class: fn(&u8) -> bool = if isnum {
            u8::is_ascii_digit
        } else {
            u8::is_ascii_alphabetic
        };
        ptr1 = one + count_while(&a[one..], class);
        ptr2 = two + count_while(&b[two..], class);
        let (seg1, seg2) = (&a[one..ptr1], &b[two..ptr2]);
        // numeric segments are always newer than alpha segments
        if seg2.is_empty() {
            return if isnum { Ordering::Greater } else { Ordering::Less };
        }
        let ord = if isnum { cmp_numeric(seg1, seg2) } else { seg1.cmp(seg2) };
        if ord != Ordering::Equal {
            return ord;
        }
        one = ptr1;
        two = ptr2;
    }
    final_showdown(&a[one.min(a.len())..], &b[two.min(b.len())..])
}

/// Decides when one side ran out of segments: a remaining alpha string never
/// beats an empty one.
fn final_showdown(rest1: &[u8], rest2: &[u8]) -> Ordering {
    if rest1.is_empty() && rest2.is_empty() {
        return Ordering::Equal;
    }
    let one_alpha = rest1.first().is_some_and(u8::is_ascii_alphabetic);
    let two_alpha = rest2.first().is_some_and(u8::is_ascii_alphabetic);
    if (rest1.is_empty() && !two_alpha) || one_alpha {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

fn cmp_numeric(a: &[u8], b: &[u8]) -> Ordering {
    let a = &a[count_while(a, |c| *c == b'0')..];
    let b = &b[count_while(b, |c| *c == b'0')..];
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn count_while(s: &[u8], pred: impl Fn(&u8) -> bool) -> usize {
    s.iter().take_while(|c| pred(c)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ord(n: i8) -> Ordering {
        n.cmp(&0)
    }

    /// Cases from upstream `test/util/vercmptest.sh`.
    const UPSTREAM: &[(&str, &str, i8)] = &[
        ("1.5.0", "1.5.0", 0),
        ("1.5.1", "1.5.0", 1),
        ("1.5.1", "1.5", 1),
        ("1.5.0-1", "1.5.0-1", 0),
        ("1.5.0-1", "1.5.0-2", -1),
        ("1.5.0-1", "1.5.1-1", -1),
        ("1.5.0-2", "1.5.1-1", -1),
        ("1.5-1", "1.5.1-1", -1),
        ("1.5-2", "1.5.1-1", -1),
        ("1.5-2", "1.5.1-2", -1),
        ("1.5", "1.5-1", 0),
        ("1.5-1", "1.5", 0),
        ("1.1-1", "1.1", 0),
        ("1.0-1", "1.1", -1),
        ("1.1-1", "1.0", 1),
        ("1.5b-1", "1.5-1", -1),
        ("1.5b", "1.5", -1),
        ("1.5b-1", "1.5", -1),
        ("1.5b", "1.5.1", -1),
        ("1.0a", "1.0alpha", -1),
        ("1.0alpha", "1.0b", -1),
        ("1.0b", "1.0beta", -1),
        ("1.0beta", "1.0rc", -1),
        ("1.0rc", "1.0", -1),
        ("1.5.a", "1.5", 1),
        ("1.5.b", "1.5.a", 1),
        ("1.5.1", "1.5.b", 1),
        ("1.5.b-1", "1.5.b", 0),
        ("1.5-1", "1.5.b", -1),
        ("2.0", "2_0", 0),
        ("2.0_a", "2_0.a", 0),
        ("2.0a", "2.0.a", -1),
        ("2___a", "2_a", 1),
        ("0:1.0", "0:1.0", 0),
        ("0:1.0", "0:1.1", -1),
        ("1:1.0", "0:1.0", 1),
        ("1:1.0", "0:1.1", 1),
        ("1:1.0", "2:1.1", -1),
        ("1:1.0", "0:1.0-1", 1),
        ("1:1.0-1", "0:1.1-1", 1),
        ("0:1.0", "1.0", 0),
        ("0:1.0", "1.1", -1),
        ("0:1.1", "1.0", 1),
        ("1:1.0", "1.0", 1),
        ("1:1.0", "1.1", 1),
        ("1:1.1", "1.1", 1),
    ];

    /// MSYS2 writes the epoch with `~`; also long numbers and leading zeros.
    const MSYS2: &[(&str, &str, i8)] = &[
        ("1~1.0-1", "2.0-1", 1),
        ("1~1.0-1", "1:1.0-1", 0),
        ("0~1.0", "1.0", 0),
        ("1~3.0-7", "1~3.0-6", 1),
        ("2~1.0", "1~9.0", 1),
        ("010", "10", 0),
        ("2", "10", -1),
        ("99999999999999999999", "99999999999999999998", 1),
    ];

    #[test]
    fn vercmp_table() {
        for &(a, b, expected) in UPSTREAM.iter().chain(MSYS2) {
            let expected = ord(expected);
            assert_eq!(vercmp(a, b), expected, "vercmp({a}, {b})");
            assert_eq!(vercmp(b, a), expected.reverse(), "vercmp({b}, {a})");
        }
    }
}
