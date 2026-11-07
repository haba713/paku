// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/pacman/pacman.c: parseargs and the parsearg_* helpers.

//! Command-line parsing with pacman's semantics: one operation letter, then
//! option letters whose meaning depends on the operation.

mod check;
pub mod getopt;
mod ops;

use getopt::{HasArg, Key, Spec};

/// The operation selected on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `-D`
    Database,
    /// `-F`
    Files,
    /// `-Q`
    Query,
    /// `-R`
    Remove,
    /// `-S`
    Sync,
    /// `-T`
    DepTest,
    /// `-U`
    Upgrade,
}

/// `--color` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// `never`
    Never,
    /// `auto`
    Auto,
    /// `always`
    Always,
}

/// Parse failure, printed differently depending on origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// getopt-level error, printed as `paku: <msg>`.
    Getopt(String),
    /// Semantic error, printed as `error: <msg>`.
    Invalid(String),
}

/// Everything parsed from the command line. Counters (`u8`) record repeated
/// flags such as `-yy` or `-ss`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    pub op: Option<Op>,
    pub help: bool,
    pub version: bool,
    // global
    pub arch: Vec<String>,
    pub ask: Option<u32>,
    pub cachedirs: Vec<String>,
    pub color: Option<ColorMode>,
    pub config: Option<String>,
    pub debug: u8,
    pub gpgdir: Option<String>,
    pub hookdirs: Vec<String>,
    pub logfile: Option<String>,
    pub noconfirm: Option<bool>,
    pub dbpath: Option<String>,
    pub root: Option<String>,
    pub sysroot: Option<String>,
    pub disable_dl_timeout: bool,
    pub verbose: u8,
    // shared between operations
    pub quiet: bool,
    pub list: bool,
    pub groups: u8,
    pub info: u8,
    pub search: bool,
    pub check: u8,
    // query
    pub changelog: bool,
    pub deps: bool,
    pub explicit: bool,
    pub foreign: bool,
    pub native: bool,
    pub owns: bool,
    pub isfile: bool,
    pub unrequired: u8,
    pub upgrades: bool,
    // transaction (-R -S -U)
    pub nodeps: u8,
    pub dbonly: bool,
    pub noprogressbar: bool,
    pub noscriptlet: bool,
    pub print: bool,
    pub print_format: Option<String>,
    pub assume_installed: Vec<String>,
    // remove
    pub cascade: bool,
    pub nosave: bool,
    pub recursive: u8,
    pub unneeded: bool,
    // upgrade (-S -U)
    pub overwrite: Vec<String>,
    pub asdeps: bool,
    pub asexplicit: bool,
    pub needed: bool,
    pub ignore: Vec<String>,
    pub ignoregroup: Vec<String>,
    pub downloadonly: bool,
    // sync
    pub clean: u8,
    pub sysupgrade: u8,
    pub refresh: u8,
    // files
    pub regex: bool,
    pub machinereadable: bool,

    pub targets: Vec<String>,
}

use HasArg::{No, Optional, Required};

/// pacman's `optstring` `DFQRSTUVb:cdefghiklmnopqr:stuvwxy`.
const SHORTS: &[(char, HasArg)] = &[
    ('D', No),
    ('F', No),
    ('Q', No),
    ('R', No),
    ('S', No),
    ('T', No),
    ('U', No),
    ('V', No),
    ('b', Required),
    ('c', No),
    ('d', No),
    ('e', No),
    ('f', No),
    ('g', No),
    ('h', No),
    ('i', No),
    ('k', No),
    ('l', No),
    ('m', No),
    ('n', No),
    ('o', No),
    ('p', No),
    ('q', No),
    ('r', Required),
    ('s', No),
    ('t', No),
    ('u', No),
    ('v', No),
    ('w', No),
    ('x', No),
    ('y', No),
];

/// pacman's long option table.
const LONGS: &[(&str, HasArg)] = &[
    ("database", No),
    ("files", No),
    ("query", No),
    ("remove", No),
    ("sync", No),
    ("deptest", No),
    ("upgrade", No),
    ("version", No),
    ("help", No),
    ("dbpath", Required),
    ("cascade", No),
    ("changelog", No),
    ("clean", No),
    ("nodeps", No),
    ("deps", No),
    ("explicit", No),
    ("groups", No),
    ("info", No),
    ("check", No),
    ("list", No),
    ("foreign", No),
    ("native", No),
    ("nosave", No),
    ("owns", No),
    ("file", No),
    ("print", No),
    ("quiet", No),
    ("root", Required),
    ("sysroot", Required),
    ("recursive", No),
    ("search", No),
    ("regex", No),
    ("machinereadable", No),
    ("unrequired", No),
    ("upgrades", No),
    ("sysupgrade", No),
    ("unneeded", No),
    ("verbose", No),
    ("downloadonly", No),
    ("refresh", No),
    ("noconfirm", No),
    ("confirm", No),
    ("config", Required),
    ("ignore", Required),
    ("assume-installed", Required),
    ("debug", Optional),
    ("force", No),
    ("overwrite", Required),
    ("noprogressbar", No),
    ("noscriptlet", No),
    ("ask", Required),
    ("cachedir", Required),
    ("hookdir", Required),
    ("asdeps", No),
    ("logfile", Required),
    ("ignoregroup", Required),
    ("needed", No),
    ("asexplicit", No),
    ("arch", Required),
    ("print-format", Required),
    ("gpgdir", Required),
    ("dbonly", No),
    ("color", Required),
    ("disable-download-timeout", No),
];

const SPEC: Spec = Spec {
    shorts: SHORTS,
    longs: LONGS,
};

/// Parses `args` (without the program name).
pub fn parse(args: &[String]) -> Result<Args, ArgError> {
    let parsed = SPEC.parse(args).map_err(ArgError::Getopt)?;
    let mut out = Args::default();
    parse_operation(&parsed.opts, &mut out)?;
    if out.help || out.version {
        return Ok(out);
    }
    for opt in &parsed.opts {
        if operation_of(opt.key).is_some() {
            continue;
        }
        let value = opt.value.as_deref();
        if ops::parse_for_op(&mut out, opt.key, value)? {
            continue;
        }
        if !ops::parse_global(&mut out, opt.key, value)? {
            return Err(ArgError::Invalid(invalid_option(opt.key)));
        }
    }
    out.targets = parsed.targets;
    check::check(&out)?;
    Ok(out)
}

/// First pass: exactly one operation, plus `-h`/`-V` (port of `parsearg_op`).
fn parse_operation(opts: &[getopt::Opt], out: &mut Args) -> Result<(), ArgError> {
    let mut seen = 0;
    for opt in opts {
        match opt.key {
            Key::Short('V') | Key::Long("version") => out.version = true,
            Key::Short('h') | Key::Long("help") => out.help = true,
            key => {
                if let Some(op) = operation_of(key) {
                    seen += 1;
                    out.op = Some(op);
                }
            }
        }
    }
    if seen > 1 {
        return Err(ArgError::Invalid("only one operation may be used at a time".into()));
    }
    Ok(())
}

fn operation_of(key: Key) -> Option<Op> {
    Some(match key {
        Key::Short('D') | Key::Long("database") => Op::Database,
        Key::Short('F') | Key::Long("files") => Op::Files,
        Key::Short('Q') | Key::Long("query") => Op::Query,
        Key::Short('R') | Key::Long("remove") => Op::Remove,
        Key::Short('S') | Key::Long("sync") => Op::Sync,
        Key::Short('T') | Key::Long("deptest") => Op::DepTest,
        Key::Short('U') | Key::Long("upgrade") => Op::Upgrade,
        _ => return None,
    })
}

fn invalid_option(key: Key) -> String {
    match key {
        Key::Short(c) => format!("invalid option '-{c}'"),
        Key::Long(name) => format!("invalid option '--{name}'"),
    }
}

#[cfg(test)]
mod tests;
