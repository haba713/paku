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
    if !p.depends.is_empty() {
        s.push_str(&format!("%DEPENDS%\n{}\n\n", p.depends.join("\n")));
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
};
const LIB: Pkg = Pkg {
    name: "lib",
    version: "2.0-1",
    depends: &[],
    files: &[("bin/lib.dll", b"lib")],
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
