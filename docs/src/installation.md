# Installation

## Prerequisites

- Windows x64 or ARM64
- [Git for Windows](https://gitforwindows.org/), which provides Git Bash
- no administrator rights

## Install script

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

### Install a specific release

Set `PAKU_VERSION`:

```bash
curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | PAKU_VERSION=v0.1.0 sh
```

## Add paku to PATH

If `~/.local/bin` is not on your `PATH` yet, the script prints how to add it. Add it once to your
Windows user `PATH`, so Git Bash, cmd and PowerShell all find it:

1. Start menu: open "Edit environment variables for your account".
2. Select `Path`, click Edit, then New, and enter `%USERPROFILE%\.local\bin`.
3. Click OK twice and open a new Git Bash window.

## Manual installation

1. Download `paku-x86_64.exe` (x64) or `paku-aarch64.exe` (ARM64) and `SHA256SUMS` from the
   [latest release](https://github.com/haba713/paku/releases/latest).
2. Verify:

   ```bash
   sha256sum -c --ignore-missing SHA256SUMS
   ```

3. Copy the binary to `~/.local/bin/paku.exe`.

## Build from source

Only a Rust toolchain is needed; no Visual Studio:

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
export RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu
git clone https://github.com/haba713/paku.git
cd paku && cargo build --release && cargo test --workspace
mkdir -p ~/.local/bin && cp target/release/paku.exe ~/.local/bin/
```

## Code signing

The release binaries are not code-signed yet. On Windows 11 with Smart App Control enabled, Windows
may block them.
