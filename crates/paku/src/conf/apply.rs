// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of _parse_options / _parse_repo in src/pacman/conf.c.

//! Applying config-file entries and command-line options onto [`Config`].

use std::path::PathBuf;

use super::ini::Entry;
use super::paths::expand_tilde;
use super::siglevel::SigLevelSetting;
use super::{Config, Repo, default_repos};
use crate::args::Args;

/// Directives that are valid pacman options but have no effect in paku.
const IGNORED_OPTIONS: &[&str] = &[
    "HookDir",
    "CheckSpace",
    "UseSyslog",
    "ILoveCandy",
    "XferCommand",
    "CleanMethod",
    "LocalFileSigLevel",
    "RemoteFileSigLevel",
    "DownloadUser",
    "DisableSandbox",
];

/// Applies config file entries. Repository sections in the file replace the
/// built-in repository list.
pub fn file(conf: &mut Config, entries: &[Entry]) -> Result<(), String> {
    let mut repos: Vec<Repo> = Vec::new();
    let mut env_set = false;
    for e in entries {
        if e.section == "options" {
            env_set |= e.key == "Environment";
            option(conf, e)?;
            continue;
        }
        repo_entry(conf, &mut repos, e)?;
    }
    if !repos.is_empty() {
        conf.repos = repos;
    } else if env_set {
        conf.repos = default_repos(&conf.env);
    }
    Ok(())
}

fn option(conf: &mut Config, e: &Entry) -> Result<(), String> {
    let path = |v: &Option<String>| expand_tilde(v.as_deref().unwrap_or_default(), &conf.home);
    let list = |v: &Option<String>| -> Vec<String> {
        v.as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect()
    };
    match e.key.as_str() {
        "InstallPrefix" | "RootDir" => conf.prefix = path(&e.value),
        "DBPath" => conf.dbpath = path(&e.value),
        "CacheDir" => conf.cachedirs = vec![path(&e.value)],
        "LogFile" => conf.logfile = path(&e.value),
        "GPGDir" => conf.gpgdir = path(&e.value),
        "GitRoot" => conf.git_root = Some(path(&e.value)),
        "Environment" => conf.env = e.value.clone().unwrap_or_default(),
        "Architecture" => set_architecture(conf, e.value.as_deref()),
        "HoldPkg" => conf.hold_pkg.extend(list(&e.value)),
        "IgnorePkg" => conf.ignore_pkg.extend(list(&e.value)),
        "IgnoreGroup" => conf.ignore_group.extend(list(&e.value)),
        "NoUpgrade" => conf.no_upgrade.extend(list(&e.value)),
        "NoExtract" => conf.no_extract.extend(list(&e.value)),
        "Color" => conf.color = true,
        "NoProgressBar" => conf.noprogressbar = true,
        "VerbosePkgLists" => conf.verbose_pkg_lists = true,
        "DisableDownloadTimeout" => conf.disable_dl_timeout = true,
        "ParallelDownloads" => conf.parallel_downloads = parse_parallel(e)?,
        "SigLevel" => apply_siglevel(&mut conf.siglevel, e)?,
        key if IGNORED_OPTIONS.contains(&key) => {}
        key => conf.warnings.push(not_recognized(e, key)),
    }
    Ok(())
}

fn repo_entry(conf: &mut Config, repos: &mut Vec<Repo>, e: &Entry) -> Result<(), String> {
    let idx = match repos.iter().position(|r| r.name == e.section) {
        Some(i) => i,
        None => {
            repos.push(Repo {
                name: e.section.clone(),
                servers: Vec::new(),
                siglevel: SigLevelSetting::default(),
            });
            repos.len() - 1
        }
    };
    match e.key.as_str() {
        "Server" => repos[idx].servers.extend(e.value.clone()),
        "SigLevel" => apply_siglevel(&mut repos[idx].siglevel, e)?,
        "Usage" | "CacheServer" => {}
        key => conf.warnings.push(not_recognized(e, key)),
    }
    Ok(())
}

fn apply_siglevel(setting: &mut SigLevelSetting, e: &Entry) -> Result<(), String> {
    let v = e.value.as_deref().unwrap_or_default();
    setting
        .apply(v)
        .map_err(|token| format!("{}: invalid value for 'SigLevel' : '{token}'", e.origin))
}

/// `Architecture = auto|x86_64|aarch64`; picks the matching environment.
fn set_architecture(conf: &mut Config, value: Option<&str>) {
    match value {
        Some("aarch64") => {
            conf.arch = "aarch64".into();
            conf.env = "clangarm64".into();
        }
        Some("x86_64") => {
            conf.arch = "x86_64".into();
            conf.env = "ucrt64".into();
        }
        _ => {}
    }
}

fn parse_parallel(e: &Entry) -> Result<u32, String> {
    let v = e.value.as_deref().unwrap_or_default();
    match v.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{}: invalid value for 'ParallelDownloads' : '{v}'", e.origin)),
    }
}

fn not_recognized(e: &Entry, key: &str) -> String {
    format!(
        "config file {}: directive '{key}' in section '{}' not recognized.",
        e.origin, e.section
    )
}

/// Applies command-line overrides. `--root` moves the install prefix and,
/// like pacman, the database and log under it.
pub fn args(conf: &mut Config, a: &Args) {
    let home = conf.home.clone();
    let path = |v: &str| expand_tilde(v, &home);
    if let Some(root) = &a.root {
        conf.prefix = path(root);
        let state = conf.prefix.join("share").join("paku");
        conf.dbpath = state.join("db");
        conf.logfile = state.join("paku.log");
    }
    if let Some(arch) = a.arch.last() {
        set_architecture(conf, Some(arch));
        conf.repos = default_repos(&conf.env);
    }
    if let Some(v) = &a.dbpath {
        conf.dbpath = path(v);
    }
    if !a.cachedirs.is_empty() {
        conf.cachedirs = a.cachedirs.iter().map(|v| path(v)).collect::<Vec<PathBuf>>();
    }
    if let Some(v) = &a.logfile {
        conf.logfile = path(v);
    }
    if let Some(v) = &a.gpgdir {
        conf.gpgdir = path(v);
    }
    conf.noprogressbar |= a.noprogressbar;
    conf.disable_dl_timeout |= a.disable_dl_timeout;
    conf.ignore_pkg.extend(a.ignore.iter().cloned());
    conf.ignore_group.extend(a.ignoregroup.iter().cloned());
}

#[cfg(test)]
mod tests {
    use super::*;
    use paku_platform::HostArch;

    fn entry(section: &str, key: &str, value: Option<&str>) -> Entry {
        Entry {
            section: section.into(),
            key: key.into(),
            value: value.map(String::from),
            origin: "test:1".into(),
        }
    }

    fn conf() -> Config {
        Config::defaults(PathBuf::from("C:/h"), HostArch::X64)
    }

    #[test]
    fn options_and_repos_from_file() {
        let mut c = conf();
        let entries = [
            entry("options", "InstallPrefix", Some("~/.local")),
            entry("options", "IgnorePkg", Some("a b")),
            entry("options", "Color", None),
            entry("myrepo", "Server", Some("file:///C:/repo")),
            entry("options", "Bogus", None),
        ];
        file(&mut c, &entries).unwrap();
        assert_eq!(c.prefix, PathBuf::from("C:/h").join(".local"));
        assert_eq!(c.ignore_pkg, ["a", "b"]);
        assert!(c.color);
        assert_eq!(c.repos.len(), 1);
        assert_eq!(c.repos[0].servers, ["file:///C:/repo"]);
        assert_eq!(c.warnings.len(), 1);
    }

    #[test]
    fn siglevel_global_and_per_repo() {
        use alpm::signing::Check;
        let mut c = conf();
        let entries = [
            entry("options", "SigLevel", Some("Optional")),
            entry("r", "Server", Some("file:///C:/r")),
            entry("r", "SigLevel", Some("PackageRequired")),
        ];
        file(&mut c, &entries).unwrap();
        let level = c.siglevel_for(&c.repos[0]);
        assert_eq!((level.package, level.database), (Check::Required, Check::Optional));
        let err = file(&mut c, &[entry("options", "SigLevel", Some("Bogus"))]).unwrap_err();
        assert_eq!(err, "test:1: invalid value for 'SigLevel' : 'Bogus'");
    }

    #[test]
    fn environment_regenerates_default_repos() {
        let mut c = conf();
        file(&mut c, &[entry("options", "Environment", Some("clang64"))]).unwrap();
        assert_eq!(c.repos[0].name, "clang64");
    }

    #[test]
    fn root_rebases_state() {
        let mut c = conf();
        let a = Args {
            root: Some("C:/r".into()),
            ..Args::default()
        };
        args(&mut c, &a);
        assert_eq!(c.prefix, PathBuf::from("C:/r"));
        assert_eq!(c.dbpath, PathBuf::from("C:/r").join("share/paku/db"));
    }

    #[test]
    fn arch_override() {
        let mut c = conf();
        let a = Args {
            arch: vec!["aarch64".into()],
            ..Args::default()
        };
        args(&mut c, &a);
        assert_eq!((c.env.as_str(), c.repos[0].name.as_str()), ("clangarm64", "clangarm64"));
    }
}
