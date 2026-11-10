# Configuration

## Where files go

Files from `ucrt64/...`, `clangarm64/...` or `usr/...` in a package are installed under `~/.local`,
the usual per-user prefix (pip and Claude Code use it too):

| In package          | Installed to          |
|---------------------|-----------------------|
| `ucrt64/bin/rg.exe` | `~/.local/bin/rg.exe` |
| `ucrt64/share/...`  | `~/.local/share/...`  |
| `usr/lib/...`       | `~/.local/lib/...`    |
| `etc/...` (msys)    | `~/.local/etc/...`    |

Programs find their DLLs and data relative to their own location, so the tree keeps its shape.

paku's own files:

| Purpose       | Default                        | Option       |
|---------------|--------------------------------|--------------|
| config        | `~/.config/paku/paku.conf`     | `--config`   |
| databases     | `~/.local/share/paku/db`       | `--dbpath`   |
| package cache | `~/.cache/paku/pkg`            | `--cachedir` |
| keyring       | `~/.local/share/paku/keyring`  | `--gpgdir`   |
| log           | `~/.local/share/paku/paku.log` | `--logfile`  |

`--root` changes the install prefix; the databases and log then move to `<root>/share/paku`.

## paku.conf

`~/.config/paku/paku.conf` is optional and uses `pacman.conf` syntax (`[options]`, `[repo]`
sections, `Include`, `Server`, `IgnorePkg`, `NoExtract`, `NoUpgrade`, `ParallelDownloads`,
`Color`, ...). paku adds these keys:

| Key             | Meaning                                                           |
|-----------------|-------------------------------------------------------------------|
| `InstallPrefix` | install prefix, default `~/.local`                                |
| `Environment`   | MSYS2 environment, default `ucrt64` (x64) or `clangarm64` (ARM64) |
| `GitRoot`       | Git for Windows directory, default found via `git.exe` on `PATH`  |

Example:

```ini
[options]
InstallPrefix = ~/.local
ParallelDownloads = 5
IgnorePkg = rsync
Color
```

## Signatures

`paku -Sy` downloads each repository's `<repo>.db.sig` and checks it; `paku -S` checks every
package's signature after download and before anything is extracted. Signatures are checked
against MSYS2's keyring, which is built into paku. A database that fails the check is deleted, and
a failing package stops the whole transaction:

```text
error: msys: key "5F944B027F7FE2091985AA2EFA11531AA0AA7F57" is unknown
error: failed to update msys (invalid or corrupted database (PGP signature))
```

`SigLevel` works as in `pacman.conf`, in `[options]` or per repository. The default is `Required`
for databases and packages, as in MSYS2's own `pacman.conf`:

| Value      | Effect                                                     |
|------------|------------------------------------------------------------|
| `Required` | every database or package must have a valid signature      |
| `Optional` | check signatures that exist; accept unsigned files         |
| `Never`    | do not check signatures                                    |

Prefix a value with `Package` or `Database` to set only that part, e.g.
`SigLevel = Required DatabaseOptional`. `TrustedOnly` and `TrustAll` are accepted and ignored:
paku trusts every key in its keyring that is not revoked.

To use a newer MSYS2 keyring than the built-in one, put `msys2.gpg` and `msys2-revoked` from the
`msys2-keyring` package into `~/.local/share/paku/keyring` (`GPGDir`, `--gpgdir`).

## Environment variables

| Variable       | Used for                                             |
|----------------|------------------------------------------------------|
| `HOME`         | home directory; falls back to `USERPROFILE`          |
| `USERPROFILE`  | home directory when `HOME` is not a Windows path     |
| `PATH`         | finding Git for Windows via `git.exe`                |
| `COLUMNS`      | output width                                         |
| `PAKU_VERSION` | `install.sh` only: release to install, e.g. `v0.1.0` |

Downloads use Windows' built-in WinHTTP, so the system's certificates and proxy settings apply.

## Limitations

- `msys` programs resolve absolute POSIX paths (`/usr/share/...`, `/etc/...`) against Git's install
  directory, not `~/.local`. Programs that need their data files there won't find them; prefer
  mingw packages.
- `.pc` files, `*-config` scripts and libtool `.la` files contain hardcoded `/ucrt64` paths; paku
  does not rewrite them.
- Install scriptlets (`.INSTALL`) assume `/` is the MSYS2 root. A failing scriptlet is a warning,
  not an abort; `--noscriptlet` skips them.
- Release binaries are not code-signed yet; Smart App Control may block them.
- Not implemented yet: replacing packages (`Replaces`) and saving modified config files as
  `.pacsave`.
