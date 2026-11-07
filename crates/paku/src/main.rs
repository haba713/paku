// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of main() in src/pacman/pacman.c.

//! `paku`: pacman-compatible, per-user package manager for Git for Windows.

mod args;
mod conf;
mod ops;
mod session;
mod ui;

use std::io::BufRead;
use std::process::ExitCode;

use args::{ArgError, Args};
use paku_platform::{Stream, is_terminal};
use ui::msg;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(&argv) as u8)
}

fn run(argv: &[String]) -> i32 {
    let mut args = match args::parse(argv) {
        Ok(a) => a,
        Err(e) => return report_arg_error(e),
    };
    if args.help {
        print!("{}", ui::help::usage(args.op));
        return 0;
    }
    if args.version {
        print!("{}", ui::help::version());
        return 0;
    }
    let Some(op) = args.op else {
        msg::error("no operation specified (use -h for help)");
        return 1;
    };
    if let Err(e) = read_stdin_targets(&mut args) {
        msg::error(&e);
        return 1;
    }
    let conf = match conf::Config::load(&args) {
        Ok(c) => c,
        Err(e) => {
            msg::error(&e);
            return 1;
        }
    };
    ui::color::init(conf.use_color(args.color, is_terminal(Stream::Stdout)));
    conf.warnings.iter().for_each(|w| msg::warning(w));
    match session::Session::open(args, conf) {
        Ok(mut s) => ops::run(op, &mut s),
        Err(e) => {
            msg::error(&e);
            1
        }
    }
}

fn report_arg_error(e: ArgError) -> i32 {
    match e {
        ArgError::Getopt(m) => eprintln!("paku: {m}"),
        ArgError::Invalid(m) => msg::error(&m),
    }
    1
}

/// Replaces a `-` target with lines read from stdin (when stdin is not a terminal).
fn read_stdin_targets(args: &mut Args) -> Result<(), String> {
    let Some(pos) = args.targets.iter().position(|t| t == "-") else {
        return Ok(());
    };
    if is_terminal(Stream::Stdin) {
        return Ok(());
    }
    args.targets.remove(pos);
    let mut found = false;
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| format!("failed to read arguments from stdin: {e}"))?;
        let line = line.trim_end_matches('\r');
        if !line.is_empty() {
            args.targets.push(line.to_string());
            found = true;
        }
    }
    if !found {
        return Err("argument '-' specified with empty stdin".into());
    }
    Ok(())
}
