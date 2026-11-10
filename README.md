# paku — pacman for Git Bash: install MSYS2 packages on Windows without admin rights

**paku** installs packages from the official [MSYS2](https://www.msys2.org/) repositories into
**Git Bash** (Git for Windows), per user under `~/.local`. Same commands as `pacman`. One
`paku.exe`, for Windows x64 and ARM64.

**Documentation:** <https://haba713.github.io/paku/>

## Install

In Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | sh
```

If `~/.local/bin` is not on your `PATH`, the script tells you how to add it.

## Use

```bash
paku -Sy                # download the package databases
paku -Ss rsync          # search
paku -S rsync           # install
paku -Syu               # upgrade all
paku -Rs rsync          # remove
```

## Status

Early development: install, upgrade, remove and signature checks work; see the
[documentation](https://haba713.github.io/paku/configuration.html#limitations) for what is missing.
paku is not affiliated with MSYS2, Arch Linux's pacman or Git for Windows.

## Contributing

Bug reports and pull requests are welcome. Read [AGENTS.md](AGENTS.md) first: architecture, build,
conventions and commit style.

## License

[GPL-2.0-or-later](LICENSE.md), as a port of pacman.
