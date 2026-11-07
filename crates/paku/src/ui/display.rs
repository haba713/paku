// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of display helpers in src/pacman/util.c.

//! Text layout helpers: wrapped lists, indented text, human-readable sizes.
//! Everything writes into a `String` so output can be tested.

use std::fmt::Write;

use super::color::colors;

/// Output width: `COLUMNS`, else 80 on a terminal, else 0 (no wrapping).
pub fn getcols() -> usize {
    if let Some(c) = std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()) {
        return c;
    }
    if paku_platform::is_terminal(paku_platform::Stream::Stdout) {
        80
    } else {
        0
    }
}

/// Prints `title value` with word wrapping; `None` for empty values.
pub fn string_display(out: &mut String, title: &str, value: &str, cols: usize) {
    let c = colors();
    let _ = write!(out, "{}{title}{} ", c.title, c.nocolor);
    if value.is_empty() {
        out.push_str("None");
    } else {
        indentprint(out, value, title.chars().count() + 1, cols);
    }
    out.push('\n');
}

/// Prints items separated by two spaces, wrapping under the title.
pub fn list_display(out: &mut String, title: &str, items: &[String], cols: usize) {
    let c = colors();
    let len = title.chars().count() + 1;
    let _ = write!(out, "{}{title}{} ", c.title, c.nocolor);
    let Some((first, rest)) = items.split_first() else {
        out.push_str("None\n");
        return;
    };
    out.push_str(first);
    let mut col = len + first.chars().count();
    for item in rest {
        let s = item.chars().count();
        if cols > len && col + s + 2 >= cols {
            out.push('\n');
            out.push_str(&" ".repeat(len));
            col = len;
        } else if col != len {
            out.push_str("  ");
            col += 2;
        }
        out.push_str(item);
        col += s;
    }
    out.push('\n');
}

/// Prints one item per line, aligned under the first.
pub fn list_display_linebreak(out: &mut String, title: &str, items: &[String], cols: usize) {
    let c = colors();
    let len = title.chars().count() + 1;
    let _ = write!(out, "{}{title}{} ", c.title, c.nocolor);
    if items.is_empty() {
        out.push_str("None\n");
        return;
    }
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(&" ".repeat(len));
        }
        indentprint(out, item, len, cols);
        out.push('\n');
    }
}

/// Word-wraps `s` at `cols`, indenting continuation lines by `indent`.
pub fn indentprint(out: &mut String, s: &str, indent: usize, cols: usize) {
    if cols == 0 || indent > cols {
        out.push_str(s);
        return;
    }
    let mut col = indent;
    let mut words = s.split(' ').filter(|w| !w.is_empty());
    if let Some(first) = words.next() {
        out.push_str(first);
        col += first.chars().count();
    }
    for word in words {
        let len = word.chars().count();
        if len + 1 > cols.saturating_sub(col) {
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            col = indent;
        } else {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += len;
    }
}

const SIZE_LABELS: [&str; 9] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];

/// Scales `bytes` to the first unit where it is within ±2048 (or to `target`).
pub fn humanize_size(bytes: i64, target: Option<&str>) -> (f64, &'static str) {
    let mut val = bytes as f64;
    let mut index = 0;
    while index < SIZE_LABELS.len() - 1 {
        let reached = match target {
            Some(t) => SIZE_LABELS[index] == t,
            None => (-2048.0..=2048.0).contains(&val),
        };
        if reached {
            break;
        }
        val /= 1024.0;
        index += 1;
    }
    (val, SIZE_LABELS[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(humanize_size(1500, None), (1500.0, "B"));
        assert_eq!(humanize_size(4096, None), (4.0, "KiB"));
        assert_eq!(humanize_size(3 * 1024 * 1024, None), (3.0, "MiB"));
        assert_eq!(humanize_size(1024 * 1024, Some("KiB")), (1024.0, "KiB"));
    }

    #[test]
    fn list_wraps_under_title() {
        let mut out = String::new();
        let items: Vec<String> = ["aaaa", "bbbb", "cccc"].iter().map(|s| s.to_string()).collect();
        list_display(&mut out, "T :", &items, 16);
        assert_eq!(out, "T : aaaa  bbbb\n    cccc\n");
        out.clear();
        list_display(&mut out, "T :", &[], 16);
        assert_eq!(out, "T : None\n");
    }

    #[test]
    fn indent_wraps_words() {
        let mut out = String::new();
        indentprint(&mut out, "one two three four", 4, 14);
        assert_eq!(out, "one two\n    three four");
        out.clear();
        indentprint(&mut out, "one two", 4, 0);
        assert_eq!(out, "one two");
    }

    #[test]
    fn linebreak_list() {
        let mut out = String::new();
        let items = vec!["a: x".to_string(), "b: y".to_string()];
        list_display_linebreak(&mut out, "Opt :", &items, 0);
        assert_eq!(out, "Opt : a: x\n      b: y\n");
    }
}
