# paku

Rust port of MSYS2's `pacman` for **Git for Windows (Git Bash)**. Same command-line interface as
pacman, same MSYS2 package repositories, but installs **per user under `~/.local`**
(programs in `~/.local/bin`), so it never needs an elevated (admin) shell.

## Goals

- command-line compatibility with MSYS2 `pacman` 6.1: same operations, same short/long options, same combined
  short-flag syntax (`-Syu`, `-Qqe`, `-Rns`), same output format where practical (scripts that parse
  `pacman -Qq`, `-Ql`, `-Si` output should work against `paku`)
- downloads from the official MSYS2 repositories (`repo.msys2.org` and mirrors), with PGP
  signatures verified against the MSYS2 keyring
- Windows **x64** and **arm64** hosts
- no writes outside the user's install prefix and paku's own state dirs; never needs admin
- single self-contained `paku.exe`; no runtime dependency on `bash`, `curl`, `gpg`, `zstd`, `tar`

## Non-goals

Out of scope for v1:

- replacing or updating Git for Windows' own runtime (`msys2-runtime`, `bash`, `filesystem`, ...)
- `makepkg` / building packages, `pacman-key` keyring management UI, `repo-add`
- the MSYS2-only "core update" flow (kill all msys processes, update runtime first)
- libalpm hooks (`/usr/share/libalpm/hooks`); parsed and skipped with a warning

## License

pacman is GPL-2.0-or-later, and paku ports its code, so paku as a whole is
**GPL-2.0-or-later**. That is the most permissive option upstream allows: "or later" lets users
pick GPLv3 (which also makes Apache-2.0 dependencies compatible). Keep it; never narrow it to
GPL-2.0-only or GPL-3.0-only.

All crates, including original code with no pacman-derived logic, and `install.sh` use the same
license; there are no MIT parts.

- Every source file starts with `// SPDX-License-Identifier: GPL-2.0-or-later` (`#` in shell
  scripts). Files ported from pacman also keep the upstream copyright lines, e.g.
  `// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>`.
- One `LICENSE.md` (GPL-2.0 text) at the repo root; every crate's `Cargo.toml` has
  `license = "GPL-2.0-or-later"`.
- Dependencies must be GPL-2.0-or-later compatible: MIT, BSD, ISC, Zlib, Apache-2.0 (via GPLv3),
  MPL-2.0. Check with `cargo deny check licenses`.

## Reference implementation

Upstream source is not vendored. Clone it outside the repo when needed:

```sh
git clone --depth 1 https://github.com/msys2/msys2-pacman.git /tmp/paku-ref/msys2-pacman
```

Treat the clone as reference material only (do not build or run anything from it). Key files:

| Upstream file                         | What to take from it                                         |
|---------------------------------------|--------------------------------------------------------------|
| `src/pacman/pacman.c`                 | `parseargs`, `parsearg_*`, long-option table, help text      |
| `src/pacman/conf.c`, `ini.c`          | `pacman.conf` parsing, `Include`, `$repo`/`$arch`            |
| `src/pacman/<op>.c`                   | operation front-ends (sync, query, remove, upgrade, ...)     |
| `src/pacman/callback.c`, `util.c`     | progress bars, prompts, `-Si`/`-Qi` output, print-format     |
| `lib/libalpm/version.c`               | `alpm_pkg_vercmp` (port exactly, incl. MSYS `~` epoch)       |
| `lib/libalpm/be_sync.c`, `be_local`   | sync DB / local DB on-disk formats                           |
| `lib/libalpm/be_package.c`            | reading `.PKGINFO`, `.MTREE`, `.INSTALL` from archives       |
| `lib/libalpm/deps.c`, `graph.c`       | dependency resolution, provides/conflicts/replaces, toposort |
| `lib/libalpm/sync.c`, `add`, `remove` | transaction prepare/commit, sysupgrade, backup files         |
| `lib/libalpm/conflict.c`, `filelist`  | file conflicts (MSYS: case-insensitive, `foo`/`foo.exe`)     |
| `lib/libalpm/signing.c`, `dload.c`    | SigLevel semantics, parallel downloads, mirror fallback      |

MSYS2-specific patches are under `#ifdef __MSYS__` — grep for them; most matter for us:

- `version.c`: epoch separator in versions is `~` as well as `:` (`grep 1~3.0-7`).
- `filelist.c`: file list comparison is ASCII case-insensitive.
- `conflict.c`: renamed `foo.exe` → `foo` is not a conflict if both are the same file of the old pkg.
- `pacman.c`: no root check; strip `\r` when reading targets from stdin (`-`).

## Platform facts

### Repositories

| Repo         | URL path under `https://repo.msys2.org` | Arch                   | Archive prefix |
|--------------|-----------------------------------------|------------------------|----------------|
| `msys`       | `/msys/x86_64/`                         | x86_64 (aarch64 empty) | `usr/`, `etc/` |
| `ucrt64`     | `/mingw/ucrt64/`                        | x86_64                 | `ucrt64/`      |
| `clang64`    | `/mingw/clang64/`                       | x86_64                 | `clang64/`     |
| `clangarm64` | `/mingw/clangarm64/`                    | aarch64                | `clangarm64/`  |
| `mingw64`    | `/mingw/mingw64/`                       | x86_64                 | `mingw64/`     |

- Each repo dir has `<repo>.db` (zstd tar), `<repo>.db.sig`, `<repo>.files`, `<repo>.files.sig`,
  packages `*.pkg.tar.zst` (older: `.pkg.tar.xz`) with detached `*.sig`.
- Mirror lists: use `https://mirror.msys2.org/...` (redirector) first, then `repo.msys2.org`.
  Ship default mirror lists equivalent to MSYS2's `mirrorlist.msys` / `mirrorlist.mingw`.
- Filenames use `~` for epoch (`:` is invalid on Windows); DB `VERSION` fields may use either.

### Architecture selection

- Detect the **native** machine with `IsWow64Process2` (Git Bash on arm64 is often x64 emulated;
  `uname -m`, `PROCESSOR_ARCHITECTURE` and `GetNativeSystemInfo` can report x86_64 there).
- Default environment: x64 → `ucrt64`; arm64 → `clangarm64`. Override with `--arch` / config.
- `msys` repo is x86_64 on both (runs emulated on arm64).
- One mingw environment per install prefix. Mixing `ucrt64` and `clangarm64` in one prefix is an
  error (their files collide in `<prefix>/bin`).

### Git for Windows baseline

- Git for Windows puts `~/bin` on `PATH`, but not `~/.local/bin`. Like Claude Code's installer,
  paku never edits shell profiles or environment variables; it tells the user what to add. When
  `<prefix>/bin` is not on `PATH`, `install.sh` and paku (after installing) print the
  `export PATH="$HOME/.local/bin:$PATH"` line to add to `~/.bashrc`. Not the Windows user `PATH`:
  msys programs need DLLs from Git's `usr/bin` (`msys-2.0.dll`, `msys-iconv-2.dll`, ...), which
  only Git Bash has on its `PATH`, so from cmd or PowerShell they fail to start.
- `/etc/package-versions.txt` lists the MSYS2 packages bundled with Git (`name version` per line).
  Load it as a read-only **virtual local DB** ("provided by Git"): satisfies dependencies, never
  upgraded or removed, not listed by `-Q` (so `-Qq` output only covers paku's own packages),
  but shown as `[installed]` in `-Ss`/`-Sl`.
- Never install `msys2-runtime*` (a second `msys-2.0.dll` breaks every msys process),
  `bash`, `filesystem`, `pacman`, `mintty` (mirror of `alpm_pkg_is_core_package`). Treat them as
  provided; refuse explicit targets with a clear error.

## Install layout

Package paths are relocated by stripping the environment prefix and joining onto `InstallPrefix`
(default `~/.local`, the usual per-user prefix also used by pip and Claude Code):

| In archive                                                   | Installed to                            |
|--------------------------------------------------------------|-----------------------------------------|
| `<env>/bin/foo.exe`, `usr/bin/foo.exe`                       | `~/.local/bin/foo.exe`                  |
| `<env>/lib/`, `<env>/share/`, `<env>/include/`, `<env>/etc/` | `~/.local/lib/`, `~/.local/share/`, ... |
| `usr/lib/`, `usr/share/`                                     | `~/.local/lib/`, `~/.local/share/`      |
| `etc/` (msys)                                                | `~/.local/etc/`                         |

`<env>` is `ucrt64`, `clangarm64`, etc.

Why one prefix tree (not executables in `~/bin` and data elsewhere): mingw programs are relocatable
and locate data as `<exe dir>/../share`, `../lib`, `../etc`; DLLs are found next to the exe. Keeping the tree shape
intact keeps them working. `~/.local` keeps the home directory itself clean. `InstallPrefix` is
configurable.

paku's own state. With the default prefix, the database and log sit in `<prefix>/share/paku`;
`--root R` moves them to `R/share/paku`:

| Purpose         | Default                               | pacman equivalent            |
|-----------------|---------------------------------------|------------------------------|
| config          | `~/.config/paku/paku.conf`            | `/etc/pacman.conf`           |
| mirror lists    | `~/.config/paku/mirrorlist.*`         | `/etc/pacman.d/mirrorlist.*` |
| local + sync DB | `~/.local/share/paku/db/{local,sync}` | `/var/lib/pacman`            |
| package cache   | `~/.cache/paku/pkg`                   | `/var/cache/pacman/pkg`      |
| keyring         | `~/.local/share/paku/keyring`         | `/etc/pacman.d/gnupg`        |
| log             | `~/.local/share/paku/paku.log`        | `/var/log/pacman.log`        |

The keyring is an embedded copy of the `msys2-keyring` package (`crates/alpm/keyring/`, see its
`README.md` for updating); `msys2.gpg` and `msys2-revoked` in `GPGDir` override it. Trust model:
every key in the keyring that is not revoked is trusted (the keyring is the trust anchor, as after
`pacman-key --populate`); certifications by the master keys are not checked. Supported: v4 RSA keys,
v4 binary-document signatures over SHA-256/384/512 — what MSYS2 uses. `SigLevel` defaults to
`Required` for databases and packages, like MSYS2's `pacman.conf`.

`--root`, `--dbpath`, `--cachedir`, `--config`, `--gpgdir`, `--logfile` override these exactly as
in pacman. `--sysroot` is accepted but only remaps paku's paths (no chroot on Windows).

### Relocation limitations

Document these for users; don't hide them.

- msys-repo programs resolve absolute POSIX paths (`/usr/share/...`, `/etc/...`) against **Git's**
  install dir, not `~`. Programs that need their data files there won't find them. Prefer mingw
  packages; warn when installing an msys package that ships files outside `usr/bin`.
- `.pc` files, `*-config` scripts, and libtool `.la` files contain hardcoded `/ucrt64` paths. Do not
  rewrite file contents in v1.
- Install scriptlets (`.INSTALL`) assume `/` is the MSYS2 root. Run them with Git's `bash` from the
  prefix dir; a failing scriptlet is a warning, not an abort. `--noscriptlet` skips them.

## CLI

Must match pacman.

Operations: `-D/--database`, `-F/--files`, `-Q/--query`, `-R/--remove`, `-S/--sync`,
`-T/--deptest`, `-U/--upgrade`, `-V/--version`, `-h/--help`.

Global options: `-b/--dbpath`, `-r/--root`, `-v/--verbose`, `--arch`, `--ask`, `--cachedir`,
`--color`, `--config`, `--confirm`, `--debug[=n]`, `--disable-download-timeout`, `--gpgdir`,
`--hookdir`, `--logfile`, `--noconfirm`, `--sysroot`.

Transaction options (`-R`, `-S`, `-U`): `-d/--nodeps` (twice = skip all checks), `--assume-installed`,
`--dbonly`, `--noprogressbar`, `--noscriptlet`, `-p/--print`, `--print-format`.
Upgrade options (`-S`, `-U`): `--asdeps`, `--asexplicit`, `--ignore`, `--ignoregroup`, `--needed`,
`--overwrite`.

Per-operation letters (same letter means different things per operation — this is why we parse
like pacman, not with a generic subcommand parser):

- `-Q`: `-c` changelog, `-d` deps, `-e` explicit, `-g` groups, `-i` info (twice: backup files),
  `-k` check (twice: mtree), `-l` list, `-m` foreign, `-n` native, `-o` owns, `-p` file, `-q` quiet,
  `-s` search, `-t` unrequired (twice), `-u` upgrades
- `-R`: `-c` cascade, `-n` nosave, `-s` recursive (twice), `-u` unneeded
- `-S`: `-c` clean (twice), `-g` groups, `-i` info (twice), `-l` list, `-q` quiet, `-s` search,
  `-u` sysupgrade (twice: allow downgrade), `-w` downloadonly, `-y` refresh (twice: force)
- `-F`: `-l` list, `-q` quiet, `-x` regex, `-y` refresh, `--machinereadable`
- `-D`: `-k` check (twice), `-q` quiet, `--asdeps`, `--asexplicit`

Other behavior:

- Exit codes, `-` (read targets from stdin, strip `\r`), `repo/pkg` targets, version constraints
  (`foo>=1.2`), groups, and `--print-format` placeholders (`%n %v %d %l %r %s %a`) behave as upstream.
- Help and error text follow upstream wording with `pacman` replaced by `paku`.
- Accept `pacman.conf` syntax in `paku.conf` (`[options]`, `[repo]`, `Include`, `Server`,
  `SigLevel`, `IgnorePkg`, `NoExtract`, `NoUpgrade`, `ParallelDownloads`, `Color`, ...), plus paku
  keys `InstallPrefix` and `Environment`.

## On-disk formats

Keep these pacman-compatible.

- **Sync DB**: tar (zstd/gz/xz/none, detect by magic) of `<name>-<ver>/desc` (and `files` in
  `.files` DBs). `desc` = `%KEY%` line, values one per line, blank line terminator. Keys: `FILENAME`,
  `NAME`, `BASE`, `VERSION`, `DESC`, `GROUPS`, `CSIZE`, `ISIZE`, `SHA256SUM`, `PGPSIG`, `URL`,
  `LICENSE`, `ARCH`, `BUILDDATE`, `PACKAGER`, `DEPENDS`, `OPTDEPENDS`, `MAKEDEPENDS`, `CHECKDEPENDS`,
  `PROVIDES`, `CONFLICTS`, `REPLACES`. Ignore unknown keys.
- **Local DB**: directory per package `local/<name>-<ver>/{desc,files,mtree,install}` +
  `local/ALPM_DB_VERSION` (9), same as libalpm, but `files` lists **relocated** paths relative to
  `InstallPrefix`; store the original archive path too (extra `%PAKU_ORIGPATH%`-style section or
  a mapping rule) so `-Ql` can print upstream-style paths.
- **Package**: `.PKGINFO` (`key = value`), `.MTREE` (gzip mtree, sha256/size per file), `.BUILDINFO`,
  optional `.INSTALL`. Verify `SHA256SUM` from the sync DB and the detached signature before extract.
- **Version compare**: port `alpm_pkg_vercmp` / `rpmvercmp` exactly; table-driven tests from
  upstream `test/util/vercmptest.sh`.

## Windows-specific behavior

- Paths case-insensitive for conflict/ownership checks (`-Qo`, file conflicts), case-preserving on disk.
- Symlinks in packages: Windows symlinks need privilege or Developer Mode. Try symlink, fall back to
  copying the target (same as MSYS `winsymlinks:deepcopy` default). Record what was done.
- Files in use: an `.exe`/`.dll` that is running can't be overwritten but can be renamed. Rename to
  `<file>.paku-old`, install new, delete leftovers on next run.
- Atomicity: extract into a staging dir inside the prefix's volume, then rename into place;
  on failure roll back what was moved. Lock with `db.lck` like pacman.
- Use long-path-aware APIs (`\\?\` prefix via `std::fs` on canonicalized paths) — some packages
  exceed 260 chars.
- Never call anything requiring elevation. If a target path is outside `InstallPrefix` after
  normalisation (`..`, absolute paths in archives) → hard error (path traversal).

## Architecture

Cargo workspace mirroring pacman's lib/front-end split, plus small crates for code with no pacman
counterpart:

```
crates/
  paku-layout/     archive path -> install path relocation (the only place building install paths)
  paku-platform/   native arch detection, file-in-use handling, symlink fallback, long paths
  paku-formats/    desc / .PKGINFO / mtree / package-versions.txt parsers, archive decompression
  alpm/            libalpm equivalent: no printing, no prompts; returns events via callbacks
    src/version.rs   vercmp
    src/db/          sync.rs, local.rs, git_provided.rs
    src/pkg.rs       package archive reading + extraction
    src/deps.rs      resolver, provides/conflicts/replaces, topo sort
    src/trans/       prepare.rs, commit.rs, add.rs, remove.rs, conflict.rs
    src/download.rs  parallel downloads, mirrors, resume, timeouts
    src/signing/     OpenPGP packets, keyring, verification, SigLevel
  paku/            binary, pacman front-end equivalent
    src/args/        getopt-style parser + per-op option structs
    src/conf/       paku.conf / Include parsing, defaults, home and Git root lookup
    src/session.rs   arguments + config + loaded databases shared by operations
    src/ops/         database, deptest, files, query, remove, sync, upgrade
    src/ui/          progress bars, prompts, colors, print-format, -Si/-Qi formatting
```

Crates in use:

- no HTTP or TLS crate: downloads use WinHTTP through `paku-platform::http` (system TLS,
  certificate store and proxy settings, follows redirects)
- `ruzstd`, `flate2` (Rust backend), `lzma-rs` for decompression; `tar`; `regex`; `httpdate`
- `thiserror` in libraries; no `windows-sys`: kernel32 and winhttp calls are declared in
  `paku-platform` (`ffi.rs`, `http/ffi.rs`); temp dirs come from `paku_platform::TempDir`
  (no `tempfile`)
- `sha2` for checksums and signature digests, `sha1` for OpenPGP key fingerprints; RSA signature
  checks via Windows CNG (`bcrypt`, `paku-platform::rsa`) behind a minimal OpenPGP parser in
  `alpm::signing` (no rPGP: it pulls in `getrandom`)
- planned: `cargo-deny` as a dev tool

Rule: no dependency may need a C compiler, `dlltool` or the Windows SDK (no `ring`, `windows-sys`
0.61+, `getrandom` 0.3+), so the project builds with the `x86_64-pc-windows-gnu` toolchain alone.

No `clap` for the main parser: pacman's operation-dependent letters and `-ss`/`-yy`/`-uu` counting
are simpler to port directly from `parseargs`.

## Build and test

```sh
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal -c clippy -c rustfmt
export RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu   # or `rustup default` it
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

No Visual Studio or Windows SDK needed: the GNU toolchain bundles its linker and import libraries.
It produces an x64 `paku.exe`, which also runs on ARM64 Windows 11 under x64 emulation and still
installs native `clangarm64` packages there. A native ARM64 binary needs the MSVC ARM64 build
tools and a Windows SDK (`--target aarch64-pc-windows-msvc`), or llvm-mingw. If rustc reports
"link: extra operand", it found Git's coreutils `/usr/bin/link` instead of an MSVC linker.

- Unit tests: vercmp, desc/PKGINFO/mtree parsing, arg parsing (table of argv → parsed op/options,
  incl. error cases), layout relocation, dependency resolution.
- Port relevant upstream `test/pacman/tests/*.py` scenarios as Rust integration tests against a
  local fixture repo (`file://` server, test-signed packages) and a temp `InstallPrefix`.
- Tests must never touch the real `~`, real repos, or the network unless marked `#[ignore]`
  (`cargo test -- --ignored` runs live-repo smoke tests).
- Compare output with real pacman where available (MSYS2 install) for `-Si`, `-Qi`, `-Ql`, `-Ss`.

## Conventions

- Modular files by responsibility; functions under ~50 lines; guard clauses over nesting.
- rustdoc (`///`) on public items; inline comments only for non-obvious *why*, e.g. citing the
  upstream function a block is ported from (`// port of _alpm_sync_prepare`).
- Library never panics on bad input (DBs, archives, config are untrusted): return errors.
- Relocation logic lives only in `paku-layout`; nothing else builds install paths.
- Never put `install`, `setup`, `update` or `patch` in an executable's name, including test
  targets (`tests/*.rs` become `<name>-<hash>.exe`): Windows installer detection then demands
  elevation ("The requested operation requires elevation", os error 740).
- Documentation and license files are Markdown with a `.md` extension (`README.md`, `AGENTS.md`,
  `LICENSE.md`); no extensionless `LICENSE` or `README`.
- Commits: conventional commits, single lowercase line, noun-first subject
  (`feat: sync db parser`, not `feat: add sync db parser`).
- Branches: `<initials>/<type>/<very-short-description>`, where `<type>` is a Conventional Commits
  type (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, ...) and the description is kebab-case,
  e.g. `haba713/feat/package-install`.
- Versioning: [Semantic Versioning](https://semver.org/). The version lives once in the workspace
  `Cargo.toml` (`[workspace.package] version`) and all crates share it. Before 1.0, breaking
  changes bump the minor version; from 1.0 on, the major version.
- Releases: bump the workspace version, commit, then push tag `vX.Y.Z`.
  `.github/workflows/release.yml` checks that the tag matches the version, then builds and tests
  `paku-x86_64.exe` (GNU toolchain) and `paku-aarch64.exe` (MSVC on `windows-11-arm`), and
  publishes them with `SHA256SUMS`. `install.sh` depends on these asset names; keep them stable.

## Open questions

- Whether to rewrite hardcoded `/ucrt64` paths in `.pc` / `*-config` files at install time.
- Whether to support msys-repo packages beyond simple `usr/bin` tools, given the `/usr` resolution
  limitation above.
- Man pages: Git for Windows ships no `man`, `groff` or `/usr/share/man` (`git help` opens HTML),
  and MSYS2's `man-db`/`groff` hit the `/usr` resolution limitation above. Installed man pages are
  therefore unreadable. Leaning towards a built-in default `NoExtract = share/man/*` (matched
  against relocated paths) that users can override in `paku.conf`, to save space.
