// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>
// Port of src/pacman/conf.c (option handling and repository setup).

//! Effective configuration: built-in defaults, `paku.conf`, then command line.

mod apply;
pub mod ini;
pub mod paths;
pub mod siglevel;

use std::path::PathBuf;

use alpm::signing::SigLevel;
use paku_platform::{HostArch, native_arch};

use self::siglevel::SigLevelSetting;
use crate::args::{Args, ColorMode};

/// One configured repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// Repository name (`ucrt64`, `msys`, ...).
    pub name: String,
    /// Server URLs with `$repo`/`$arch` still unexpanded.
    pub servers: Vec<String>,
    /// The repository's own `SigLevel` lines.
    pub siglevel: SigLevelSetting,
}

/// Effective configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub home: PathBuf,
    pub config_file: PathBuf,
    /// Install prefix (`InstallPrefix` / `RootDir` / `--root`).
    pub prefix: PathBuf,
    pub dbpath: PathBuf,
    pub cachedirs: Vec<PathBuf>,
    pub logfile: PathBuf,
    pub gpgdir: PathBuf,
    /// Git for Windows root, for the bundled-package manifest.
    pub git_root: Option<PathBuf>,
    /// MSYS2 mingw environment (`ucrt64`, `clangarm64`, ...).
    pub env: String,
    /// Native architecture name (`x86_64`, `aarch64`).
    pub arch: String,
    pub repos: Vec<Repo>,
    pub hold_pkg: Vec<String>,
    pub ignore_pkg: Vec<String>,
    pub ignore_group: Vec<String>,
    pub no_upgrade: Vec<String>,
    pub no_extract: Vec<String>,
    pub color: bool,
    pub noprogressbar: bool,
    pub verbose_pkg_lists: bool,
    pub parallel_downloads: u32,
    pub disable_dl_timeout: bool,
    /// `SigLevel` from `[options]`.
    pub siglevel: SigLevelSetting,
    /// Non-fatal config problems to print as warnings.
    pub warnings: Vec<String>,
}

impl Config {
    /// Built-in defaults for `home` on a host with architecture `arch`.
    pub fn defaults(home: PathBuf, arch: HostArch) -> Self {
        let prefix = home.join(".local");
        let state = prefix.join("share").join("paku");
        let env = arch.default_env().to_string();
        Config {
            config_file: home.join(".config").join("paku").join("paku.conf"),
            prefix,
            dbpath: state.join("db"),
            cachedirs: vec![home.join(".cache").join("paku").join("pkg")],
            logfile: state.join("paku.log"),
            gpgdir: state.join("keyring"),
            git_root: None,
            repos: default_repos(&env),
            env,
            arch: arch.msys2_name().to_string(),
            hold_pkg: Vec::new(),
            ignore_pkg: Vec::new(),
            ignore_group: Vec::new(),
            no_upgrade: Vec::new(),
            no_extract: Vec::new(),
            color: false,
            noprogressbar: false,
            verbose_pkg_lists: false,
            parallel_downloads: 5,
            disable_dl_timeout: false,
            siglevel: SigLevelSetting::default(),
            warnings: Vec::new(),
            home,
        }
    }

    /// Loads the effective configuration for parsed command-line `args`.
    pub fn load(args: &Args) -> Result<Self, String> {
        let home = paths::home_dir().ok_or("could not determine home directory")?;
        let mut conf = Config::defaults(home, native_arch());
        if let Some(path) = &args.config {
            conf.config_file = paths::expand_tilde(path, &conf.home);
        }
        if args.config.is_some() || conf.config_file.is_file() {
            let entries = ini::read(&conf.config_file)?;
            apply::file(&mut conf, &entries)?;
        }
        apply::args(&mut conf, args);
        conf.make_absolute();
        if conf.git_root.is_none() {
            conf.git_root = paths::find_git_root();
        }
        Ok(conf)
    }

    /// Resolves relative paths (e.g. `--root target/x`) against the current directory,
    /// so output and the database always show absolute paths, as pacman does.
    fn make_absolute(&mut self) {
        let abs = |p: &mut PathBuf| {
            if let Ok(a) = std::path::absolute(&*p) {
                *p = a;
            }
        };
        abs(&mut self.prefix);
        abs(&mut self.dbpath);
        abs(&mut self.logfile);
        abs(&mut self.gpgdir);
        self.cachedirs.iter_mut().for_each(abs);
    }

    /// Server URLs of `repo` with `$repo` and `$arch` expanded.
    pub fn expanded_servers(&self, repo: &Repo) -> Vec<String> {
        // MSYS2 publishes the msys repository for x86_64 only; arm64 hosts run it emulated.
        let arch = if repo.name == "msys" {
            "x86_64"
        } else {
            self.arch.as_str()
        };
        repo.servers
            .iter()
            .map(|s| s.replace("$repo", &repo.name).replace("$arch", arch))
            .collect()
    }

    /// Effective signature level of `repo`: its own `SigLevel` over the global one,
    /// over the built-in default (`Required`).
    pub fn siglevel_for(&self, repo: &Repo) -> SigLevel {
        repo.siglevel.over(self.siglevel.over(SigLevel::default()))
    }

    /// Whether colors are enabled for this run.
    pub fn use_color(&self, mode: Option<ColorMode>, is_tty: bool) -> bool {
        match mode {
            Some(ColorMode::Always) => true,
            Some(ColorMode::Never) => false,
            Some(ColorMode::Auto) => is_tty,
            None => self.color && is_tty,
        }
    }
}

/// Built-in repositories: the mingw environment first, then `msys`
/// (same order as MSYS2's `pacman.conf`).
pub fn default_repos(env: &str) -> Vec<Repo> {
    let mingw = |repo: &str| Repo {
        name: repo.to_string(),
        servers: vec![
            "https://mirror.msys2.org/mingw/$repo/".into(),
            "https://repo.msys2.org/mingw/$repo/".into(),
        ],
        siglevel: SigLevelSetting::default(),
    };
    let msys = Repo {
        name: "msys".into(),
        servers: vec![
            "https://mirror.msys2.org/msys/$arch/".into(),
            "https://repo.msys2.org/msys/$arch/".into(),
        ],
        siglevel: SigLevelSetting::default(),
    };
    vec![mingw(env), msys]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_architecture() {
        let c = Config::defaults(PathBuf::from("C:/h"), HostArch::Arm64);
        assert_eq!(c.env, "clangarm64");
        assert_eq!(c.repos[0].name, "clangarm64");
        assert_eq!(c.repos[1].name, "msys");
        assert_eq!(c.prefix, PathBuf::from("C:/h").join(".local"));
        assert_eq!(c.dbpath, PathBuf::from("C:/h").join(".local/share/paku/db"));
    }

    #[test]
    fn msys_arch_is_always_x86_64() {
        let c = Config::defaults(PathBuf::from("C:/h"), HostArch::Arm64);
        assert_eq!(
            c.expanded_servers(&c.repos[0])[0],
            "https://mirror.msys2.org/mingw/clangarm64/"
        );
        assert_eq!(
            c.expanded_servers(&c.repos[1])[1],
            "https://repo.msys2.org/msys/x86_64/"
        );
    }
}
