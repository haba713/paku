# MSYS2 keyring

Embedded into paku as the default keyring for package and database signatures.

| File            | Content                                                        |
|-----------------|----------------------------------------------------------------|
| `msys2.gpg`     | ASCII-armored public keys of MSYS2's master and packager keys  |
| `msys2-revoked` | fingerprints of revoked keys, one per line                     |

- source: `usr/share/pacman/keyrings/` of the MSYS2 package `msys2-keyring` `1~20260814-1`
  ([MSYS2-keyring](https://github.com/msys2/MSYS2-keyring))
- license: GPL-3.0-or-later (paku's GPL-2.0-or-later allows combining under GPLv3)
- `msys2-trusted` is not used: paku trusts every key in this keyring that is not revoked

To update, install the newest `msys2-keyring` into a throwaway prefix
(`paku --root <tmp> -Sy msys2-keyring`), copy `msys2.gpg` and `msys2-revoked` from
`<tmp>/share/pacman/keyrings/` here, update the version above, and run the tests. Users can also
override the embedded copy by putting the same two files into paku's `GPGDir` (default
`~/.local/share/paku/keyring`).
