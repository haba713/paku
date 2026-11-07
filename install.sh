#!/bin/sh
# SPDX-License-Identifier: MIT
#
# Installs paku into ~/.local/bin. No admin rights needed.
#
#   curl -fsSL https://raw.githubusercontent.com/haba713/paku/master/install.sh | sh
#
# Set PAKU_VERSION (e.g. v0.1.0) to install a specific release instead of the latest.

set -eu

main() {
    case "$(uname -s)" in
        *ARM64*) arch=aarch64 ;;
        MINGW* | MSYS*) arch=x86_64 ;;
        *) echo "error: run this in Git Bash on Windows" >&2; exit 1 ;;
    esac

    if [ -n "${PAKU_VERSION:-}" ]; then
        url="https://github.com/haba713/paku/releases/download/$PAKU_VERSION"
    else
        url="https://github.com/haba713/paku/releases/latest/download"
    fi
    exe="paku-$arch.exe"
    dir="$HOME/.local/bin"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    cd "$tmp"

    echo "Downloading $exe ..."
    curl -fsSL -O "$url/$exe" -O "$url/SHA256SUMS"
    grep " \*\{0,1\}$exe\$" SHA256SUMS | sha256sum -c --quiet -

    mkdir -p "$dir"
    mv -f "$exe" "$dir/paku.exe"
    echo "Installed $dir/paku.exe"

    case ":$PATH:" in
        *":$dir:"*) ;;
        *)
            echo
            echo "Add $(cygpath -w "$dir") to your Windows user PATH:"
            echo "  1. Start menu: open \"Edit environment variables for your account\"."
            echo "  2. Select \"Path\", click Edit, then New, and enter the directory above."
            echo "  3. Click OK twice and open a new Git Bash window."
            ;;
    esac
}

# Running everything from main() means a partially downloaded script never runs.
main
