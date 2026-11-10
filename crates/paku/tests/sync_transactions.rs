// SPDX-License-Identifier: GPL-2.0-or-later

//! End-to-end install tests: a fixture repository served over `file://`, the real
//! `paku` binary, and a temporary install root. Nothing touches the real home
//! directory, the network or Git's package list.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use paku_platform::TempDir;

struct Pkg {
    name: &'static str,
    version: &'static str,
    depends: &'static [&'static str],
    files: &'static [(&'static str, &'static [u8])],
    /// Config files (`backup = ...`), relative to the prefix like `files`.
    backup: &'static [&'static str],
    replaces: &'static [&'static str],
    provides: &'static [&'static str],
    conflicts: &'static [&'static str],
}

struct Fixture {
    dir: TempDir,
}

impl Fixture {
    /// Unsigned test repository with signature checks off (`SigLevel = Never`).
    fn new(pkgs: &[Pkg]) -> Self {
        Self::with_siglevel(pkgs, "Never")
    }

    /// Unsigned test repository with the given `SigLevel` for `[test]`.
    fn with_siglevel(pkgs: &[Pkg], siglevel: &str) -> Self {
        let dir = TempDir::new("paku-e2e-").unwrap();
        let repo = dir.path().join("repo");
        let conf = format!(
            "[options]\nEnvironment = ucrt64\nGitRoot = {}\nGPGDir = {}\n\n\
             [test]\nServer = file:///{}\nSigLevel = {siglevel}\n",
            dir.path().join("no-git").display(),
            dir.path().join("keyring").display(),
            repo.display().to_string().replace('\\', "/")
        );
        fs::write(dir.path().join("paku.conf"), conf).unwrap();
        let f = Fixture { dir };
        f.publish(pkgs);
        f
    }

    /// Replaces the repository contents with `pkgs`.
    fn publish(&self, pkgs: &[Pkg]) {
        let repo = self.dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let mut db = tar::Builder::new(Vec::new());
        for p in pkgs {
            let file = write_pkg(&repo, p);
            add(
                &mut db,
                &format!("{}-{}/desc", p.name, p.version),
                desc(p, &file).as_bytes(),
            );
        }
        fs::write(repo.join("test.db"), db.into_inner().unwrap()).unwrap();
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("root")
    }

    fn paku(&self, args: &[&str]) -> Output {
        let d = self.dir.path();
        Command::new(env!("CARGO_BIN_EXE_paku"))
            .args(["--config", &d.join("paku.conf").display().to_string()])
            .args(["--root", &self.root().display().to_string()])
            .args(["--cachedir", &d.join("cache").display().to_string()])
            .args(["--noconfirm", "--color", "never"])
            .args(args)
            .output()
            .unwrap()
    }
}

fn write_pkg(repo: &Path, p: &Pkg) -> PathBuf {
    let mut b = tar::Builder::new(Vec::new());
    let mut info = format!("pkgname = {}\npkgver = {}\narch = any\nsize = 1\n", p.name, p.version);
    for d in p.depends {
        info.push_str(&format!("depend = {d}\n"));
    }
    for path in p.backup {
        info.push_str(&format!("backup = ucrt64/{path}\n"));
    }
    for (key, values) in [
        ("replaces", p.replaces),
        ("provides", p.provides),
        ("conflict", p.conflicts),
    ] {
        for v in values {
            info.push_str(&format!("{key} = {v}\n"));
        }
    }
    add(&mut b, ".PKGINFO", info.as_bytes());
    add_dirs(&mut b, p.files);
    for (path, data) in p.files {
        add(&mut b, &format!("ucrt64/{path}"), data);
    }
    let path = repo.join(format!("{}-{}-any.pkg.tar", p.name, p.version));
    fs::write(&path, b.into_inner().unwrap()).unwrap();
    path
}

fn desc(p: &Pkg, file: &Path) -> String {
    let sha = alpm::checksum::sha256_file(file).unwrap();
    let size = fs::metadata(file).unwrap().len();
    let fname = file.file_name().unwrap().to_string_lossy();
    let mut s = format!(
        "%FILENAME%\n{fname}\n\n%NAME%\n{}\n\n%VERSION%\n{}\n\n%CSIZE%\n{size}\n\n%SHA256SUM%\n{sha}\n\n%ARCH%\nany\n\n",
        p.name, p.version
    );
    let lists = [
        ("DEPENDS", p.depends),
        ("REPLACES", p.replaces),
        ("PROVIDES", p.provides),
        ("CONFLICTS", p.conflicts),
    ];
    for (key, values) in lists.iter().filter(|(_, v)| !v.is_empty()) {
        s.push_str(&format!("%{key}%\n{}\n\n", values.join("\n")));
    }
    s
}

/// Directory entries for every parent of `files`, as real packages have.
fn add_dirs(b: &mut tar::Builder<Vec<u8>>, files: &[(&str, &[u8])]) {
    let mut dirs: Vec<String> = Vec::new();
    for (path, _) in files {
        let mut acc = String::from("ucrt64/");
        for part in path.split('/').rev().skip(1).collect::<Vec<_>>().into_iter().rev() {
            acc.push_str(part);
            acc.push('/');
            if !dirs.contains(&acc) {
                dirs.push(acc.clone());
            }
        }
    }
    for d in dirs {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_size(0);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, &d, &[][..]).unwrap();
    }
}

fn add(b: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8]) {
    let mut h = tar::Header::new_gnu();
    h.set_size(data.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    b.append_data(&mut h, path, data).unwrap();
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

const APP: Pkg = Pkg {
    name: "app",
    version: "1.0-1",
    depends: &["lib"],
    files: &[("bin/app.exe", b"app")],
    backup: &[],
    replaces: &[],
    provides: &[],
    conflicts: &[],
};
const LIB: Pkg = Pkg {
    name: "lib",
    version: "2.0-1",
    depends: &[],
    files: &[("bin/lib.dll", b"lib")],
    backup: &[],
    replaces: &[],
    provides: &[],
    conflicts: &[],
};

#[test]
fn installs_package_with_dependency() {
    let f = Fixture::new(&[APP, LIB]);
    assert!(f.paku(&["-Sy"]).status.success());
    let out = f.paku(&["-S", "app"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(fs::read(f.root().join("bin/app.exe")).unwrap(), b"app");
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"lib");
    let q = f.paku(&["-Q"]);
    assert_eq!(String::from_utf8_lossy(&q.stdout), "app 1.0-1\nlib 2.0-1\n");
    let deps = f.paku(&["-Qqd"]);
    assert_eq!(String::from_utf8_lossy(&deps.stdout), "lib\n");
}

#[test]
fn unsigned_database_is_rejected_by_default() {
    let f = Fixture::with_siglevel(&[APP, LIB], "Required");
    let out = f.paku(&["-Sy"]);
    assert!(!out.status.success());
    let t = text(&out);
    assert!(t.contains("error: test: missing required signature"), "{t}");
    assert!(
        t.contains("failed to update test (invalid or corrupted database (PGP signature))"),
        "{t}"
    );
    assert!(!f.root().join("share/paku/db/sync/test.db").exists());
}

#[test]
fn unsigned_package_is_rejected_before_install() {
    let f = Fixture::with_siglevel(&[APP, LIB], "DatabaseNever PackageRequired");
    assert!(f.paku(&["-Sy"]).status.success());
    let out = f.paku(&["-S", "lib"]);
    assert!(!out.status.success());
    let t = text(&out);
    assert!(t.contains("error: lib: missing required signature"), "{t}");
    assert!(t.contains("invalid or corrupted package (PGP signature)"), "{t}");
    assert!(!f.root().join("bin/lib.dll").exists());
}

#[test]
fn optional_siglevel_accepts_unsigned_repo() {
    let f = Fixture::with_siglevel(&[LIB], "Optional");
    assert!(f.paku(&["-Sy"]).status.success());
    let out = f.paku(&["-S", "lib"]);
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn untracked_file_blocks_install_unless_overwritten() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    fs::create_dir_all(f.root().join("bin")).unwrap();
    fs::write(f.root().join("bin/lib.dll"), b"mine").unwrap();

    let out = f.paku(&["-S", "app"]);
    assert!(!out.status.success());
    assert!(text(&out).contains("lib.dll exists in filesystem"), "{}", text(&out));
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"mine");
    assert!(!f.root().join("bin/app.exe").exists());

    let out = f.paku(&["-S", "app", "--overwrite", "/bin/*"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"lib");
}

#[test]
fn missing_dependency_fails_cleanly() {
    let f = Fixture::new(&[APP]);
    f.paku(&["-Sy"]);
    let out = f.paku(&["-S", "app"]);
    assert!(!out.status.success());
    assert!(
        text(&out).contains("unable to satisfy dependency 'lib' required by app"),
        "{}",
        text(&out)
    );
    assert!(!f.root().join("bin").exists());
}

#[test]
fn needed_skips_installed_package() {
    let f = Fixture::new(&[LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "lib"]).status.success());
    let out = f.paku(&["-S", "--needed", "lib"]);
    assert!(out.status.success());
    assert!(text(&out).contains("is up to date -- skipping"), "{}", text(&out));
}

#[test]
fn sysupgrade_upgrades_installed_packages() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());
    let out = f.paku(&["-Su"]);
    assert!(out.status.success());
    assert!(text(&out).contains(" there is nothing to do"), "{}", text(&out));

    const LIB2: Pkg = Pkg {
        version: "3.0-1",
        files: &[("bin/lib.dll", b"lib3"), ("bin/lib-extra.dll", b"x")],
        ..LIB
    };
    f.publish(&[APP, LIB2]);
    let out = f.paku(&["-Syu", "--ignore", "app"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("upgrading lib"), "{}", text(&out));
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"lib3");
    assert!(f.root().join("bin/lib-extra.dll").is_file());
    let q = f.paku(&["-Q"]);
    assert_eq!(String::from_utf8_lossy(&q.stdout), "app 1.0-1\nlib 3.0-1\n");
    let deps = f.paku(&["-Qqd"]);
    assert_eq!(String::from_utf8_lossy(&deps.stdout), "lib\n");

    f.publish(&[APP, LIB]);
    f.paku(&["-Sy"]);
    let out = f.paku(&["-Su"]);
    assert!(
        text(&out).contains("lib: local (3.0-1) is newer than test (2.0-1)"),
        "{}",
        text(&out)
    );
    let out = f.paku(&["-Suu"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("downgrading lib"), "{}", text(&out));
    assert!(!f.root().join("bin/lib-extra.dll").exists());
}

#[test]
fn remove_checks_dependencies_and_deletes_files() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());

    let out = f.paku(&["-R", "lib"]);
    assert!(!out.status.success());
    assert!(
        text(&out).contains(":: removing lib breaks dependency 'lib' required by app"),
        "{}",
        text(&out)
    );
    assert!(f.root().join("bin/lib.dll").is_file());

    let out = f.paku(&["-R", "nope"]);
    assert!(text(&out).contains("error: target not found: nope"), "{}", text(&out));

    let out = f.paku(&["-Rsp", "app"]);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "app-1.0-1\nlib-2.0-1\n");

    let out = f.paku(&["-Rs", "app"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("(2/2) removing lib"), "{}", text(&out));
    assert!(!f.root().join("bin").exists());
    assert!(f.paku(&["-Q"]).stdout.is_empty());
    let log = fs::read_to_string(f.root().join("share/paku/paku.log")).unwrap();
    assert!(log.contains("[ALPM] removed app (1.0-1)"), "{log}");
}

#[test]
fn remove_cascade_takes_dependents() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());
    let out = f.paku(&["-Rc", "lib"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(f.paku(&["-Q"]).stdout.is_empty());
    assert!(!f.root().join("bin/app.exe").exists());
}

const TOOL: Pkg = Pkg {
    name: "tool",
    version: "1.0-1",
    depends: &[],
    files: &[("etc/tool.conf", b"v1"), ("etc/defaults.conf", b"v1")],
    backup: &["etc/tool.conf", "etc/defaults.conf"],
    replaces: &[],
    provides: &[],
    conflicts: &[],
};
const TOOL2: Pkg = Pkg {
    version: "2.0-1",
    files: &[("etc/tool.conf", b"v2"), ("etc/defaults.conf", b"v2")],
    ..TOOL
};

#[test]
fn edited_config_file_survives_upgrade_and_removal() {
    let f = Fixture::new(&[TOOL]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "tool"]).status.success());
    let etc = f.root().join("etc");
    fs::write(etc.join("tool.conf"), b"mine").unwrap();

    f.publish(&[TOOL2]);
    let out = f.paku(&["-Syu"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("tool.conf.pacnew"), "{}", text(&out));
    assert_eq!(fs::read(etc.join("tool.conf")).unwrap(), b"mine");
    assert_eq!(fs::read(etc.join("tool.conf.pacnew")).unwrap(), b"v2");
    assert_eq!(fs::read(etc.join("defaults.conf")).unwrap(), b"v2");
    assert!(!etc.join("defaults.conf.pacnew").exists());

    let out = f.paku(&["-R", "tool"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("tool.conf.pacsave"), "{}", text(&out));
    assert_eq!(fs::read(etc.join("tool.conf.pacsave")).unwrap(), b"mine");
    assert!(!etc.join("tool.conf").exists());
    assert!(!etc.join("defaults.conf").exists());
    let log = fs::read_to_string(f.root().join("share/paku/paku.log")).unwrap();
    assert!(log.contains("[ALPM] warning: ") && log.contains(" saved as "), "{log}");
}

#[test]
fn nosave_removes_edited_config_file() {
    let f = Fixture::new(&[TOOL]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "tool"]).status.success());
    fs::write(f.root().join("etc/tool.conf"), b"mine").unwrap();
    let out = f.paku(&["-Rn", "tool"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(!f.root().join("etc").exists());
}

#[test]
fn noupgrade_file_is_left_alone() {
    let f = Fixture::new(&[LIB]);
    fs::write(
        f.dir.path().join("paku.conf"),
        fs::read_to_string(f.dir.path().join("paku.conf"))
            .unwrap()
            .replace("[options]\n", "[options]\nNoUpgrade = bin/lib.dll\n"),
    )
    .unwrap();
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "lib"]).status.success());
    const LIB3: Pkg = Pkg {
        version: "3.0-1",
        files: &[("bin/lib.dll", b"lib3")],
        ..LIB
    };
    f.publish(&[LIB3]);
    let out = f.paku(&["-Syu"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"lib");
    assert_eq!(fs::read(f.root().join("bin/lib.dll.pacnew")).unwrap(), b"lib3");
}

/// `lib` renamed to `newlib`, which takes over `bin/lib.dll`.
const NEWLIB: Pkg = Pkg {
    name: "newlib",
    version: "3.0-1",
    depends: &[],
    files: &[("bin/lib.dll", b"new"), ("bin/newlib.dll", b"x")],
    backup: &[],
    replaces: &["lib"],
    provides: &["lib"],
    conflicts: &["lib"],
};

#[test]
fn sysupgrade_replaces_renamed_package() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());

    f.publish(&[APP, NEWLIB]);
    let out = f.paku(&["-Syu"]);
    let t = text(&out);
    assert!(out.status.success(), "{t}");
    assert!(t.contains(":: Replace lib with test/newlib? [Y/n]"), "{t}");
    assert!(t.contains("lib-2.0-1 [removal]"), "{t}");
    assert!(t.contains("(1/1) removing lib"), "{t}");
    assert!(t.contains("(1/1) installing newlib"), "{t}");

    let q = f.paku(&["-Q"]);
    assert_eq!(String::from_utf8_lossy(&q.stdout), "app 1.0-1\nnewlib 3.0-1\n");
    let deps = f.paku(&["-Qqd"]);
    assert_eq!(String::from_utf8_lossy(&deps.stdout), "newlib\n", "reason is inherited");
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"new");
    assert!(f.root().join("bin/newlib.dll").is_file());
    let log = fs::read_to_string(f.root().join("share/paku/paku.log")).unwrap();
    assert!(log.contains("[ALPM] removed lib (2.0-1)"), "{log}");
    assert!(log.contains("[ALPM] installed newlib (3.0-1)"), "{log}");
}

#[test]
fn replacement_breaking_a_dependency_is_refused() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());
    const NO_PROVIDES: Pkg = Pkg {
        provides: &[],
        ..NEWLIB
    };
    f.publish(&[APP, NO_PROVIDES]);
    let out = f.paku(&["-Syu"]);
    let t = text(&out);
    assert!(!out.status.success(), "{t}");
    assert!(
        t.contains(":: removing lib breaks dependency 'lib' required by app"),
        "{t}"
    );
    assert_eq!(fs::read(f.root().join("bin/lib.dll")).unwrap(), b"lib");
    let q = f.paku(&["-Q"]);
    assert_eq!(String::from_utf8_lossy(&q.stdout), "app 1.0-1\nlib 2.0-1\n");
}

#[test]
fn ignored_replacement_is_skipped_with_a_warning() {
    let f = Fixture::new(&[APP, LIB]);
    f.paku(&["-Sy"]);
    assert!(f.paku(&["-S", "app"]).status.success());
    f.publish(&[APP, NEWLIB]);
    let out = f.paku(&["-Syu", "--ignore", "newlib"]);
    let t = text(&out);
    assert!(out.status.success(), "{t}");
    assert!(
        t.contains("ignoring package replacement (lib-2.0-1 => newlib-3.0-1)"),
        "{t}"
    );
    let q = f.paku(&["-Q"]);
    assert_eq!(String::from_utf8_lossy(&q.stdout), "app 1.0-1\nlib 2.0-1\n");
}
