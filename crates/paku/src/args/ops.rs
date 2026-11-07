// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the parsearg_* functions in src/pacman/pacman.c.

//! Per-operation option handlers. Each returns `Ok(true)` when it consumed the
//! option, `Ok(false)` to fall through, `Err` for invalid arguments.

use super::getopt::Key::{self, Long, Short};
use super::{ArgError, Args, ColorMode, Op};

type Handled = Result<bool, ArgError>;

/// Dispatches to the handler of the selected operation.
pub fn parse_for_op(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    match a.op {
        Some(Op::Database) => Ok(database(a, key)),
        Some(Op::Query) => Ok(query(a, key)),
        Some(Op::Remove) => remove(a, key, value),
        Some(Op::Sync) => sync(a, key, value),
        Some(Op::Upgrade) => upgrade(a, key, value),
        Some(Op::Files) => files(a, key, value),
        Some(Op::DepTest) | None => Ok(false),
    }
}

/// Options valid for every operation (port of `parsearg_global`).
pub fn parse_global(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    let v = || value.unwrap_or_default().to_string();
    match key {
        Long("arch") => a.arch.push(v()),
        Long("ask") => a.ask = Some(parse_ask(value)?),
        Long("cachedir") => a.cachedirs.push(v()),
        Long("color") => a.color = Some(parse_color(value)?),
        Long("config") => a.config = Some(v()),
        Long("debug") => a.debug = a.debug.max(parse_debug(value)?),
        Long("gpgdir") => a.gpgdir = Some(v()),
        Long("hookdir") => a.hookdirs.push(v()),
        Long("logfile") => a.logfile = Some(v()),
        Long("noconfirm") => a.noconfirm = Some(true),
        Long("confirm") => a.noconfirm = Some(false),
        Long("dbpath") | Short('b') => a.dbpath = Some(v()),
        Long("root") | Short('r') => a.root = Some(v()),
        Long("sysroot") => a.sysroot = Some(v()),
        Long("disable-download-timeout") => a.disable_dl_timeout = true,
        Long("verbose") | Short('v') => a.verbose += 1,
        _ => return Ok(false),
    }
    if key == Long("debug") {
        // progress bars get garbled with debug output
        a.noprogressbar = true;
    }
    Ok(true)
}

fn database(a: &mut Args, key: Key) -> bool {
    match key {
        Long("asdeps") => a.asdeps = true,
        Long("asexplicit") => a.asexplicit = true,
        Long("check") | Short('k') => a.check += 1,
        Long("quiet") | Short('q') => a.quiet = true,
        _ => return false,
    }
    true
}

fn query(a: &mut Args, key: Key) -> bool {
    match key {
        Long("changelog") | Short('c') => a.changelog = true,
        Long("deps") | Short('d') => a.deps = true,
        Long("explicit") | Short('e') => a.explicit = true,
        Long("groups") | Short('g') => a.groups += 1,
        Long("info") | Short('i') => a.info += 1,
        Long("check") | Short('k') => a.check += 1,
        Long("list") | Short('l') => a.list = true,
        Long("foreign") | Short('m') => a.foreign = true,
        Long("native") | Short('n') => a.native = true,
        Long("owns") | Short('o') => a.owns = true,
        Long("file") | Short('p') => a.isfile = true,
        Long("quiet") | Short('q') => a.quiet = true,
        Long("search") | Short('s') => a.search = true,
        Long("unrequired") | Short('t') => a.unrequired += 1,
        Long("upgrades") | Short('u') => a.upgrades = true,
        _ => return false,
    }
    true
}

/// Options common to `-R`, `-S`, `-U` (and accepted by `-F`, as upstream).
fn trans(a: &mut Args, key: Key, value: Option<&str>) -> bool {
    match key {
        Long("nodeps") | Short('d') => a.nodeps += 1,
        Long("dbonly") => {
            a.dbonly = true;
            a.noscriptlet = true;
        }
        Long("noprogressbar") => a.noprogressbar = true,
        Long("noscriptlet") => a.noscriptlet = true,
        Long("print") | Short('p') => a.print = true,
        Long("print-format") => {
            a.print = true;
            a.print_format = value.map(str::to_string);
        }
        Long("assume-installed") => add_list(&mut a.assume_installed, value),
        _ => return false,
    }
    true
}

fn remove(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    if trans(a, key, value) {
        return Ok(true);
    }
    match key {
        Long("cascade") | Short('c') => a.cascade = true,
        Long("nosave") | Short('n') => a.nosave = true,
        Long("recursive") | Short('s') => a.recursive += 1,
        Long("unneeded") | Short('u') => a.unneeded = true,
        _ => return Ok(false),
    }
    Ok(true)
}

/// Options common to `-S` and `-U` (port of `parsearg_upgrade`).
fn upgrade(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    if trans(a, key, value) {
        return Ok(true);
    }
    match key {
        Long("overwrite") => add_list(&mut a.overwrite, value),
        Long("asdeps") => a.asdeps = true,
        Long("asexplicit") => a.asexplicit = true,
        Long("needed") => a.needed = true,
        Long("ignore") => add_list(&mut a.ignore, value),
        Long("ignoregroup") => add_list(&mut a.ignoregroup, value),
        Long("downloadonly") | Short('w') => a.downloadonly = true,
        _ => return Ok(false),
    }
    Ok(true)
}

fn sync(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    if upgrade(a, key, value)? {
        return Ok(true);
    }
    match key {
        Long("clean") | Short('c') => a.clean += 1,
        Long("groups") | Short('g') => a.groups += 1,
        Long("info") | Short('i') => a.info += 1,
        Long("list") | Short('l') => a.list = true,
        Long("quiet") | Short('q') => a.quiet = true,
        Long("search") | Short('s') => a.search = true,
        Long("sysupgrade") | Short('u') => a.sysupgrade += 1,
        Long("refresh") | Short('y') => a.refresh += 1,
        _ => return Ok(false),
    }
    Ok(true)
}

fn files(a: &mut Args, key: Key, value: Option<&str>) -> Handled {
    if trans(a, key, value) {
        return Ok(true);
    }
    match key {
        Long("list") | Short('l') => a.list = true,
        Long("refresh") | Short('y') => a.refresh += 1,
        Long("regex") | Short('x') => a.regex = true,
        Long("machinereadable") => a.machinereadable = true,
        Long("quiet") | Short('q') => a.quiet = true,
        _ => return Ok(false),
    }
    Ok(true)
}

/// Splits a comma-separated option value into a list (`parsearg_util_addlist`).
fn add_list(list: &mut Vec<String>, value: Option<&str>) {
    let items = value.unwrap_or_default().split(',').filter(|s| !s.is_empty());
    list.extend(items.map(str::to_string));
}

fn parse_color(value: Option<&str>) -> Result<ColorMode, ArgError> {
    match value {
        Some("never") => Ok(ColorMode::Never),
        Some("auto") => Ok(ColorMode::Auto),
        Some("always") => Ok(ColorMode::Always),
        v => Err(ArgError::Invalid(format!(
            "invalid argument '{}' for --color",
            v.unwrap_or_default()
        ))),
    }
}

fn parse_debug(value: Option<&str>) -> Result<u8, ArgError> {
    match value {
        None | Some("1") => Ok(1),
        Some("2") => Ok(2),
        Some(v) => Err(ArgError::Invalid(format!("'{v}' is not a valid debug level"))),
    }
}

fn parse_ask(value: Option<&str>) -> Result<u32, ArgError> {
    let v = value.unwrap_or_default();
    v.parse()
        .map_err(|_| ArgError::Invalid(format!("'{v}' is not a valid ask level")))
}
