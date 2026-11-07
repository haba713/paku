# paku

pacman-compatible package manager for Git Bash: installs MSYS2 packages into `~/.local`, no admin
rights needed.

paku brings pacman to Git Bash, the shell that comes with Git for Windows. It downloads packages
from the official [MSYS2](https://www.msys2.org/) repositories and installs them per user, so you
never need an elevated shell. Commands and options are the same as pacman's.

paku is not affiliated with MSYS2, Arch Linux's pacman or Git for Windows.

> **Status: early development.** Refreshing, searching, installing and upgrading packages work
> (`-Sy`, `-Ss`, `-Si`, `-Sl`, `-Sg`, `-S <package>`, `-Syu`, `-Q`, `-T`). Package signatures are
> not verified yet (only SHA-256 checksums). Removing packages (`-R`) and replacing packages
> (`Replaces`) are not implemented yet.

## Install

In Git Bash:

```sh
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | sh
```

The script downloads the latest release binary for your machine (x64 or ARM64), checks its SHA-256
checksum and installs it as `~/.local/bin/paku.exe`.

If `~/.local/bin` is not on your `PATH` yet, the script prints how to add it. Add it once to your
Windows user `PATH`, so Git Bash, cmd and PowerShell all find it:

1. Start menu: open "Edit environment variables for your account".
2. Select `Path`, click Edit, then New, and enter `%USERPROFILE%\.local\bin`.
3. Click OK twice and open a new Git Bash window.

To install a specific release, set `PAKU_VERSION`:

```sh
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | PAKU_VERSION=v0.1.0 sh
```

`curl | sh` runs whatever the server returns. The script is about 50 lines; to read it first:

```sh
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | less
```

Or install manually:

1. Download `paku-x86_64.exe` (x64) or `paku-aarch64.exe` (ARM64) and `SHA256SUMS` from the
   [latest release](https://github.com/haba713/paku/releases/latest).
2. Verify: `sha256sum -c --ignore-missing SHA256SUMS`
3. Copy the binary to `~/.local/bin/paku.exe`.

Or build from source, see [Building](#building).

The release binaries are not code-signed yet. On Windows 11 with Smart App Control enabled, Windows
may block them.

## Usage

Example: install `rsync`, which Git Bash lacks.

```sh
paku -Sy                # download the package databases
paku -Ss '^rsync$'      # search by exact name; `paku -Ss rsync` also matches descriptions
```

```
msys/rsync 3.5.2-1 (net-utils)
    A file transfer program to keep remote files in sync
```

`msys/` is the repository. If nothing matches, `-Ss` prints nothing and exits with code 1.

```sh
paku -Si rsync          # package details
paku -S rsync           # install it and its missing dependencies
paku -Q                 # list packages installed with paku
paku -Syu               # upgrade all packages installed with paku
paku -h                 # help; paku -S -h for -S options
```

Package names: `msys` packages such as `rsync` have no prefix. Packages from the native Windows
environments carry one: `mingw-w64-ucrt-x86_64-*` on x64 (`ucrt64`) and `mingw-w64-clang-aarch64-*`
on ARM64 (`clangarm64`). paku picks the environment from the real CPU, even when Git Bash itself
runs under x64 emulation.

`msys` programs like `rsync` use the `msys-2.0.dll` runtime from Git for Windows, so run them from
Git Bash.

## How it works

- Files from `ucrt64/...`, `clangarm64/...` or `usr/...` in a package are installed under `~/.local`,
  the usual per-user prefix (pip and Claude Code use it too): `ucrt64/bin/rg.exe` becomes
  `~/.local/bin/rg.exe`, `ucrt64/share/...` becomes `~/.local/share/...`. Programs find their DLLs
  and data relative to their own location, so the tree keeps its shape. `InstallPrefix` in
  `paku.conf` changes the prefix.
- State lives in `~/.local/share/paku` (databases, log), `~/.cache/paku` (downloads) and
  `~/.config/paku/paku.conf` (optional, pacman.conf syntax).
- Packages bundled with Git for Windows (listed in `/etc/package-versions.txt`) count as installed,
  so dependencies like `libiconv` or `libopenssl` are not downloaded again. paku never replaces
  Git's own runtime (`msys2-runtime`, `bash`, ...).
- Downloads use Windows' built-in WinHTTP, so the system's certificates and proxy settings apply.

## Building

Only a Rust toolchain is needed; no Visual Studio:

```sh
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
export RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu
git clone https://github.com/haba713/paku.git
cd paku && cargo build --release && cargo test --workspace
mkdir -p ~/.local/bin && cp target/release/paku.exe ~/.local/bin/
```

See [AGENTS.md](AGENTS.md) for architecture and conventions.

## License

paku is a port of pacman and is licensed under the
[GNU General Public License v2.0 or later](LICENSE.md). The crates `paku-formats`, `paku-layout`
and `paku-platform` contain no code derived from pacman and are MIT-licensed; see the `LICENSE.md`
file in each crate's directory.
