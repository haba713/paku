# paku: pacman for Git Bash

**paku** is a pacman-compatible package manager for **Git Bash** (Git for Windows). It downloads
packages from the official [MSYS2](https://www.msys2.org/) repositories and installs them per user
into `~/.local`, so you never need an administrator shell. Commands and options match MSYS2
`pacman`: `paku -Syu`, `paku -S rsync`, `paku -Rs`, `paku -Qq`.

paku is not affiliated with MSYS2, Arch Linux's pacman or Git for Windows.

> **Status: early development.** Install, upgrade, remove and signature checks work; see
> [Limitations](configuration.md#limitations) for what is missing.

## Key features

- **pacman-compatible CLI**: same operations, options and combined short flags (`-Syu`, `-Qqe`,
  `-Rns`) as MSYS2 `pacman`.
- **No admin rights**: installs per user under `~/.local`; programs land in `~/.local/bin`.
- **Official MSYS2 repositories**: `msys`, `ucrt64` (x64) and `clangarm64` (ARM64) packages.
- **x64 and ARM64**: picks the environment from the real CPU, even when Git Bash runs under x64
  emulation.
- **Reuses Git for Windows**: packages already bundled with Git count as installed and are never
  replaced.
- **Single binary**: one `paku.exe`, no dependency on `curl`, `gpg`, `zstd` or `tar`.

## Quick start

In Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | sh
paku -Sy                # download the package databases
paku -S rsync           # install rsync and its missing dependencies
```

Next steps:

- [Installation](installation.md): prerequisites, `PATH` setup, manual install
- [Usage](usage.md): searching, installing, upgrading and removing packages
- [Configuration](configuration.md): where files go, `paku.conf`, limitations

## More information

- [Source code](https://github.com/haba713/paku): GitHub repository
- [AGENTS.md](https://github.com/haba713/paku/blob/master/AGENTS.md): architecture and conventions
- [Issues](https://github.com/haba713/paku/issues): bug reports and feature requests
- [License](https://github.com/haba713/paku/blob/master/LICENSE.md): GPL-2.0-or-later
