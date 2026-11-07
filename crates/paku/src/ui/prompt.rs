// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of yesno / question in src/pacman/util.c.

//! Yes/no questions.

use std::io::{self, BufRead, Write};

use super::color::colors;

/// Asks `:: <question> [Y/n]` (default yes). With `noconfirm` the default is taken
/// without reading input, as in pacman. End of input also means the default.
pub fn yesno(question: &str, noconfirm: bool) -> bool {
    ask(question, true, noconfirm)
}

/// Asks `:: <question> [y/N]` (default no).
pub fn noyes(question: &str, noconfirm: bool) -> bool {
    ask(question, false, noconfirm)
}

fn ask(question: &str, default: bool, noconfirm: bool) -> bool {
    let c = colors();
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{}{question} {hint} {}", c.colon, c.nocolor);
    if noconfirm {
        println!();
        return default;
    }
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
        println!();
        return default;
    }
    parse_answer(&line, default)
}

fn parse_answer(line: &str, default: bool) -> bool {
    match line.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "y" | "yes" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers() {
        assert!(parse_answer("\r\n", true));
        assert!(parse_answer("Y\n", true));
        assert!(parse_answer("yes", false));
        assert!(!parse_answer("n", true));
        assert!(!parse_answer("nope", true));
    }
}
