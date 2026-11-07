// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of the checkargs_* functions in src/pacman/pacman.c.

//! Rejects option combinations that pacman refuses.

use super::{ArgError, Args, Op};

type Checked = Result<(), ArgError>;

/// Validates option combinations for the selected operation.
pub fn check(a: &Args) -> Checked {
    match a.op {
        Some(Op::Database) => database(a),
        Some(Op::Sync) => sync(a),
        Some(Op::Query) => query(a),
        Some(Op::Remove) => remove(a),
        Some(Op::Upgrade) => upgrade(a),
        Some(Op::Files) | Some(Op::DepTest) | None => Ok(()),
    }
}

fn invalid(used: bool, opt1: &str, opt2: &str) -> Checked {
    if !used {
        return Ok(());
    }
    Err(ArgError::Invalid(format!(
        "invalid option: '{opt1}' and '{opt2}' may not be used together"
    )))
}

fn database(a: &Args) -> Checked {
    invalid(a.asdeps && a.asexplicit, "--asdeps", "--asexplicit")?;
    if a.check > 0 {
        invalid(a.asdeps, "--asdeps", "--check")?;
        invalid(a.asexplicit, "--asexplicit", "--check")?;
    }
    Ok(())
}

fn query_display_opts(a: &Args, opname: &str) -> Checked {
    invalid(a.changelog, opname, "--changelog")?;
    invalid(a.check > 0, opname, "--check")?;
    invalid(a.info > 0, opname, "--info")?;
    invalid(a.list, opname, "--list")
}

fn query_filter_opts(a: &Args, opname: &str) -> Checked {
    invalid(a.deps, opname, "--deps")?;
    invalid(a.explicit, opname, "--explicit")?;
    invalid(a.upgrades, opname, "--upgrade")?;
    invalid(a.unrequired > 0, opname, "--unrequired")?;
    invalid(a.native, opname, "--native")?;
    invalid(a.foreign, opname, "--foreign")
}

fn query(a: &Args) -> Checked {
    let groups = a.groups > 0;
    if a.isfile {
        invalid(groups, "--file", "--groups")?;
        invalid(a.search, "--file", "--search")?;
        invalid(a.owns, "--file", "--owns")?;
    } else if a.search {
        invalid(groups, "--search", "--groups")?;
        invalid(a.owns, "--search", "--owns")?;
        query_display_opts(a, "--search")?;
        query_filter_opts(a, "--search")?;
    } else if a.owns {
        invalid(groups, "--owns", "--groups")?;
        query_display_opts(a, "--owns")?;
        query_filter_opts(a, "--owns")?;
    } else if groups {
        query_display_opts(a, "--groups")?;
    }
    invalid(a.deps && a.explicit, "--deps", "--explicit")?;
    invalid(a.native && a.foreign, "--native", "--foreign")
}

fn trans(a: &Args) -> Checked {
    if a.print {
        invalid(a.dbonly, "--print", "--dbonly")?;
        invalid(a.noscriptlet, "--print", "--noscriptlet")?;
    }
    Ok(())
}

fn remove(a: &Args) -> Checked {
    trans(a)?;
    if a.nosave {
        invalid(a.print, "--nosave", "--print")?;
        invalid(a.dbonly, "--nosave", "--dbonly")?;
    }
    Ok(())
}

fn upgrade(a: &Args) -> Checked {
    trans(a)?;
    invalid(a.asdeps && a.asexplicit, "--asdeps", "--asexplicit")
}

fn sync(a: &Args) -> Checked {
    upgrade(a)?;
    let groups = a.groups > 0;
    if a.clean > 0 {
        invalid(groups, "--clean", "--groups")?;
        invalid(a.info > 0, "--clean", "--info")?;
        invalid(a.list, "--clean", "--list")?;
        invalid(a.refresh > 0, "--clean", "--refresh")?;
        invalid(a.search, "--clean", "--search")?;
        invalid(a.sysupgrade > 0, "--clean", "--sysupgrade")?;
        invalid(a.downloadonly, "--clean", "--downloadonly")?;
    } else if a.info > 0 {
        invalid(groups, "--info", "--groups")?;
        invalid(a.list, "--info", "--list")?;
        invalid(a.search, "--info", "--search")?;
        invalid(a.sysupgrade > 0, "--info", "--sysupgrade")?;
        invalid(a.downloadonly, "--info", "--downloadonly")?;
    } else if a.search {
        invalid(groups, "--search", "--groups")?;
        invalid(a.list, "--search", "--list")?;
        invalid(a.sysupgrade > 0, "--search", "--sysupgrade")?;
        invalid(a.downloadonly, "--search", "--downloadonly")?;
    } else if a.list {
        invalid(groups, "--list", "--groups")?;
        invalid(a.sysupgrade > 0, "--list", "--sysupgrade")?;
        invalid(a.downloadonly, "--list", "--downloadonly")?;
    } else if groups {
        invalid(a.sysupgrade > 0, "--groups", "--sysupgrade")?;
        invalid(a.downloadonly, "--groups", "--downloadonly")?;
    }
    Ok(())
}
