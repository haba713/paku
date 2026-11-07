# Usage

paku takes the same operations and options as MSYS2 `pacman`. Run `paku -h` for help and
`paku -S -h` for the options of one operation.

## Example: install rsync

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

```bash
paku -Si rsync          # package details
paku -S rsync           # install it and its missing dependencies
```

## Common commands

| Command           | What it does                                             |
|-------------------|----------------------------------------------------------|
| `paku -Sy`        | download the package databases                           |
| `paku -Ss <text>` | search package names and descriptions                    |
| `paku -Si <pkg>`  | show package details                                     |
| `paku -Sl`        | list packages in the repositories                        |
| `paku -Sg`        | list package groups                                      |
| `paku -S <pkg>`   | install a package and its missing dependencies           |
| `paku -Syu`       | upgrade all packages installed with paku                 |
| `paku -Q`         | list packages installed with paku                        |
| `paku -Rs <pkg>`  | remove a package and the dependencies nothing else needs |
| `paku -T <dep>`   | check whether dependencies are satisfied                 |

Combined short flags work as in pacman (`-Syu`, `-Qqe`, `-Rns`), and `-` reads targets from stdin.

## Package names

| Repository   | Host CPU | Package name prefix        |
|--------------|----------|----------------------------|
| `msys`       | both     | none, e.g. `rsync`         |
| `ucrt64`     | x64      | `mingw-w64-ucrt-x86_64-`   |
| `clangarm64` | ARM64    | `mingw-w64-clang-aarch64-` |

paku picks the environment from the real CPU, even when Git Bash itself runs under x64 emulation.

`msys` programs like `rsync` use the `msys-2.0.dll` runtime from Git for Windows, so run them from
Git Bash.

## Packages bundled with Git

Packages bundled with Git for Windows (listed in `/etc/package-versions.txt`) count as installed, so
dependencies like `libiconv` or `libopenssl` are not downloaded again. They are not listed by `-Q`.
paku never replaces Git's own runtime (`msys2-runtime`, `bash`, `filesystem`, `pacman`, `mintty`)
and refuses them as explicit targets.
