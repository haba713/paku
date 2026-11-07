# paku — pacman for Git Bash: install MSYS2 packages on Windows without admin rights

**paku** is a pacman-compatible package manager for **Git Bash** (Git for Windows). It downloads
packages from the official [MSYS2](https://www.msys2.org/) repositories and installs them per user
into `~/.local`, so you never need an elevated (administrator) shell. Commands and options are the
same as MSYS2 `pacman`: `paku -Syu`, `paku -S rsync`, `paku -Rs`, `paku -Qq`. paku is a single
self-contained `paku.exe` written in Rust, for Windows x64 and ARM64.

paku is not affiliated with MSYS2, Arch Linux's pacman or Git for Windows.

> **Status: early development.** Refreshing, searching, installing, upgrading and removing packages
> work (`-Sy`, `-Ss`, `-Si`, `-Sl`, `-Sg`, `-S <package>`, `-Syu`, `-R`, `-Q`, `-T`). Package
> signatures are not verified yet (only SHA-256 checksums). Replacing packages (`Replaces`) and
> saving modified config files as `.pacsave` are not implemented yet.

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Installation](#installation)
  - [Install script](#install-script)
  - [Add paku to PATH](#add-paku-to-path)
  - [Install a specific release](#install-a-specific-release)
  - [Manual installation](#manual-installation)
  - [Code signing](#code-signing)
- [Quick Start / Usage](#quick-start--usage)
  - [Example: install rsync](#example-install-rsync)
  - [Common commands](#common-commands)
  - [Package names and environments](#package-names-and-environments)
- [Architecture / How It Works](#architecture--how-it-works)
  - [Install layout](#install-layout)
  - [State and configuration](#state-and-configuration)
  - [Packages bundled with Git](#packages-bundled-with-git)
  - [Downloads](#downloads)
  - [Source layout](#source-layout)
- [Contributing](#contributing)
  - [Building from source](#building-from-source)
- [License](#license)

## Overview

paku brings pacman to Git Bash, the shell that comes with Git for Windows. Git Bash ships only a
small set of Unix tools and no package manager; paku fills that gap with the same command-line
interface as MSYS2's `pacman`, the same package repositories, and a per-user install prefix
(`~/.local`) instead of a system-wide one.

## Key Features

- **pacman-compatible CLI**: same operations, options and combined short flags (`-Syu`, `-Qqe`,
  `-Rns`) as MSYS2 `pacman`.
- **No admin rights**: installs per user under `~/.local`; programs land in `~/.local/bin`.
- **Official MSYS2 repositories**: `msys`, `ucrt64` (x64) and `clangarm64` (ARM64) packages.
- **x64 and ARM64**: picks the environment from the real CPU, even when Git Bash runs under x64
  emulation.
- **Reuses Git for Windows**: packages already bundled with Git count as installed and are never
  replaced.
- **Single binary**: one `paku.exe`, no dependency on `curl`, `gpg`, `zstd` or `tar`.
- **System networking**: downloads use Windows WinHTTP, so system certificates and proxy settings
  apply.

## Installation

### Install script

In Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | sh
```

The script downloads the latest release binary for your machine (x64 or ARM64), checks its SHA-256
checksum and installs it as `~/.local/bin/paku.exe`.

`curl | sh` runs whatever the server returns. The script is about 50 lines; to read it first:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | less
```

### Add paku to PATH

If `~/.local/bin` is not on your `PATH` yet, the script prints how to add it. Add it to Git Bash's
`PATH` in `~/.bashrc`, then open a new Git Bash window:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

Avoid adding `~/.local/bin` to the Windows user `PATH` for cmd and PowerShell: `msys` programs such
as `rsync` need DLLs from Git's `usr\bin` (`msys-2.0.dll`, `msys-iconv-2.dll`, ...), which only Git
Bash has on its `PATH`, so they fail to start there. Native Windows packages (`mingw-w64-ucrt-x86_64-*`,
`mingw-w64-clang-aarch64-*`) keep their DLLs next to the program and would work, but the same
directory holds both kinds.

### Install a specific release

Set `PAKU_VERSION`:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | PAKU_VERSION=v0.1.0 sh
```

### Manual installation

1. Download `paku-x86_64.exe` (x64) or `paku-aarch64.exe` (ARM64) and `SHA256SUMS` from the
   [latest release](https://github.com/haba713/paku/releases/latest).
2. Verify:

   ```bash
   sha256sum -c --ignore-missing SHA256SUMS
   ```

3. Copy the binary to `~/.local/bin/paku.exe`.

Or build from source, see [Building from source](#building-from-source).

### Code signing

The release binaries are not code-signed yet. On Windows 11 with Smart App Control enabled, Windows
may block them.

## Quick Start / Usage

### Example: install rsync

Install `rsync`, which Git Bash lacks:

```bash
paku -Sy                # download the package databases
paku -Ss '^rsync$'      # search by exact name; `paku -Ss rsync` also matches descriptions
```

```text
msys/rsync 3.5.2-1 (net-utils)
    A file transfer program to keep remote files in sync
```

`msys/` is the repository. If nothing matches, `-Ss` prints nothing and exits with code 1.

### Common commands

```bash
paku -Si rsync          # package details
paku -S rsync           # install it and its missing dependencies
paku -Q                 # list packages installed with paku
paku -Syu               # upgrade all packages installed with paku
paku -Rs rsync          # remove it and the dependencies nothing else needs
paku -h                 # help; paku -S -h for -S options
```

### Package names and environments

| Repository   | Host CPU | Package name prefix          | Example                         |
|--------------|----------|------------------------------|---------------------------------|
| `msys`       | both     | none                         | `rsync`                         |
| `ucrt64`     | x64      | `mingw-w64-ucrt-x86_64-`     | `mingw-w64-ucrt-x86_64-*`       |
| `clangarm64` | ARM64    | `mingw-w64-clang-aarch64-`   | `mingw-w64-clang-aarch64-*`     |

paku picks the environment from the real CPU, even when Git Bash itself runs under x64 emulation.

`msys` programs like `rsync` use the `msys-2.0.dll` runtime from Git for Windows, so run them from
Git Bash.

## Architecture / How It Works

### Install layout

Files from `ucrt64/...`, `clangarm64/...` or `usr/...` in a package are installed under `~/.local`,
the usual per-user prefix (pip and Claude Code use it too):

| In package          | Installed to          |
|---------------------|-----------------------|
| `ucrt64/bin/rg.exe` | `~/.local/bin/rg.exe` |
| `ucrt64/share/...`  | `~/.local/share/...`  |

Programs find their DLLs and data relative to their own location, so the tree keeps its shape.
`InstallPrefix` in `paku.conf` changes the prefix.

### State and configuration

| Purpose                 | Location                                                |
|-------------------------|---------------------------------------------------------|
| databases, log          | `~/.local/share/paku`                                   |
| downloads               | `~/.cache/paku`                                         |
| config (optional)       | `~/.config/paku/paku.conf` (pacman.conf syntax)         |

### Packages bundled with Git

Packages bundled with Git for Windows (listed in `/etc/package-versions.txt`) count as installed, so
dependencies like `libiconv` or `libopenssl` are not downloaded again. paku never replaces Git's own
runtime (`msys2-runtime`, `bash`, ...).

### Downloads

Downloads use Windows' built-in WinHTTP, so the system's certificates and proxy settings apply.

### Source layout

A Cargo workspace mirroring pacman's library / front-end split:

| Crate           | License          | Purpose                                                       |
|-----------------|------------------|---------------------------------------------------------------|
| `paku`          | GPL-2.0-or-later | the binary: argument parsing, config, operations, output      |
| `alpm`          | GPL-2.0-or-later | libalpm equivalent: databases, dependencies, transactions     |
| `paku-formats`  | MIT              | `desc`, `.PKGINFO`, mtree parsers, archive decompression      |
| `paku-layout`   | MIT              | archive path to install path relocation                       |
| `paku-platform` | MIT              | native CPU detection, WinHTTP, file-in-use handling, symlinks |

See [AGENTS.md](AGENTS.md) for the full architecture and conventions.

## Contributing

Bug reports and pull requests are welcome. Before you start, read [AGENTS.md](AGENTS.md): it covers
the architecture, coding conventions, licensing per crate and commit message style.

### Building from source

Only a Rust toolchain is needed; no Visual Studio:

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
export RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu
git clone https://github.com/haba713/paku.git
cd paku && cargo build --release && cargo test --workspace
mkdir -p ~/.local/bin && cp target/release/paku.exe ~/.local/bin/
```

## License

paku is a port of pacman and is licensed under the
[GNU General Public License v2.0 or later](LICENSE.md). The crates `paku-formats`, `paku-layout`
and `paku-platform` contain no code derived from pacman and are MIT-licensed; see the `LICENSE.md`
file in each crate's directory.
