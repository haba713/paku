// SPDX-License-Identifier: GPL-2.0-or-later

//! Operation front-ends (`-S`, `-Q`, `-T`, ...). Each returns the exit code.

mod deptest;
mod install;
mod query;
mod remove;
mod sync;

use crate::args::Op;
use crate::session::Session;
use crate::ui::msg;

/// Runs the selected operation.
pub fn run(op: Op, s: &mut Session) -> i32 {
    match op {
        Op::Sync => sync::run(s),
        Op::Query => query::run(s),
        Op::DepTest => deptest::run(s),
        Op::Remove => remove::run(s),
        Op::Upgrade | Op::Database | Op::Files => not_implemented(op),
    }
}

fn not_implemented(op: Op) -> i32 {
    msg::error(&format!("operation {op:?} is not implemented yet"));
    1
}

/// Splits `repo/name` targets.
pub fn split_repo(target: &str) -> (Option<&str>, &str) {
    match target.split_once('/') {
        Some((repo, name)) => (Some(repo), name),
        None => (None, target),
    }
}
