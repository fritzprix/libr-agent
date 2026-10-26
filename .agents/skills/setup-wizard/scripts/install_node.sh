#!/bin/bash
# Node.js Installation Script for Linux
#
# Uses the system package manager only when running as root or when sudo works
# without a password. Otherwise installs the official Node.js LTS binary tarball
# under ~/.local (no root needed). Never blocks on interactive input.
#
# Usage: install_node.sh [--force]   (--force reinstalls even if node exists)

set -e

FORCE=0
for arg in "$@"; do
    case $arg in
        --force|-f) FORCE=1 ;;
        *) echo "Unknown option: $arg"; echo "Usage: $0 [--force]"; exit 1 ;;
    esac
done

echo "=== Node.js Installation Script ==="
echo ""

# Check if Node.js is already installed
if command -v node &> /dev/null && [ "$FORCE" -ne 1 ]; then
    echo "Node.js already installed: $(node --version)"
    echo "Nothing to do. Re-run with --force to reinstall."
    exit 0
fi

# Decide how to get privileges: root, passwordless sudo, or none (user-local install)
SUDO=""
USE_SYSTEM_PM=0
if [ "$(id -u)" -eq 0 ]; then
    USE_SYSTEM_PM=1
elif command -v sudo &> /dev/null && sudo -n true 2> /dev/null; then
    SUDO="sudo"
    USE_SYSTEM_PM=1
fi

OS=""
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    echo "Detected OS: $OS $VERSION_ID"
    echo ""
fi

install_with_system_pm() {
    case $OS in
        ubuntu|debian)
            echo "Installing Node.js on Ubuntu/Debian..."
            curl -fsSL https://deb.nodesource.com/setup_lts.x | $SUDO -E bash -
            $SUDO apt-get install -y nodejs
            ;;

        fedora|rhel|centos)
            echo "Installing Node.js on Fedora/RHEL/CentOS..."
            curl -fsSL https://rpm.nodesource.com/setup_lts.x | $SUDO bash -
            $SUDO dnf install -y nodejs
            ;;

        arch|manjaro)
            echo "Installing Node.js on Arch Linux..."
            $SUDO pacman -S --noconfirm nodejs npm
            ;;

        opensuse*)
            echo "Installing Node.js on openSUSE..."
            $SUDO zypper install -y nodejs npm
            ;;

        *)
            return 1
            ;;
    esac
}

fetch() {
    if command -v curl &> /dev/null; then
        curl -fsSL "$1"
    elif command -v wget &> /dev/null; then
        wget -qO- "$1"
    else
        echo "Neither curl nor wget is available." >&2
        return 1
    fi
}

install_user_local() {
    echo "Installing the official Node.js LTS tarball under ~/.local (no root needed)"

    if [ -z "${HOME:-}" ] || [ ! -w "$HOME" ]; then
        echo "HOME is unset or not writable; cannot install under ~/.local."
        return 1
    fi

    case $(uname -m) in
        x86_64|amd64) NODE_ARCH=x64 ;;
        aarch64|arm64) NODE_ARCH=arm64 ;;
        armv7l) NODE_ARCH=armv7l ;;
        *) echo "Unsupported architecture: $(uname -m)"; return 1 ;;
    esac

    # Official binaries are glibc builds; musl (Alpine) needs the package manager.
    if [ "$OS" = "alpine" ]; then
        echo "Alpine (musl) is not supported by official Node.js tarballs. Run 'apk add nodejs npm' as root."
        return 1
    fi

    NODE_VERSION=$(fetch https://nodejs.org/dist/index.json | grep -m1 '"lts":"' | sed -E 's/.*"version":"(v[^"]+)".*/\1/')
    if [ -z "$NODE_VERSION" ]; then
        echo "Could not determine the latest Node.js LTS version."
        return 1
    fi
    echo "Latest LTS: $NODE_VERSION ($NODE_ARCH)"

    PREFIX="$HOME/.local"
    DEST="$PREFIX/lib/nodejs/node-$NODE_VERSION-linux-$NODE_ARCH"
    TARBALL="node-$NODE_VERSION-linux-$NODE_ARCH.tar.gz"
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    fetch "https://nodejs.org/dist/$NODE_VERSION/$TARBALL" > "$TMP_DIR/$TARBALL"

    if command -v sha256sum &> /dev/null; then
        EXPECTED=$(fetch "https://nodejs.org/dist/$NODE_VERSION/SHASUMS256.txt" | grep " $TARBALL\$" | awk '{print $1}')
        ACTUAL=$(sha256sum "$TMP_DIR/$TARBALL" | awk '{print $1}')
        if [ -z "$EXPECTED" ] || [ "$EXPECTED" != "$ACTUAL" ]; then
            echo "Checksum verification failed for $TARBALL."
            return 1
        fi
        echo "Checksum verified."
    fi

    rm -rf "$DEST"
    mkdir -p "$DEST" "$PREFIX/bin"
    tar -xzf "$TMP_DIR/$TARBALL" -C "$DEST" --strip-components=1
    for bin in node npm npx corepack; do
        if [ -e "$DEST/bin/$bin" ]; then
            ln -sfn "$DEST/bin/$bin" "$PREFIX/bin/$bin"
        fi
    done

    # Idempotently add ~/.local/bin to the shell rc files
    PATH_LINE='export PATH="$HOME/.local/bin:$PATH"'
    for rc in "$HOME/.bashrc" "$HOME/.profile" "$HOME/.zshrc"; do
        if [ "$rc" = "$HOME/.zshrc" ] && [ ! -f "$rc" ]; then
            continue
        fi
        if ! grep -qsF "$PATH_LINE" "$rc"; then
            printf '\n# Added by LibrAgent setup-wizard\n%s\n' "$PATH_LINE" >> "$rc"
        fi
    done

    export PATH="$PREFIX/bin:$PATH"
    echo ""
    echo "Installed to: $DEST"
    echo "PATH line (already appended to your shell rc files; add it manually to other shells):"
    echo "  $PATH_LINE"
}

if [ "$USE_SYSTEM_PM" -eq 1 ] && install_with_system_pm; then
    :
elif [ "$USE_SYSTEM_PM" -eq 1 ]; then
    echo "No system package manager recipe for: ${OS:-unknown}; falling back to the user-local install."
    install_user_local || {
        echo "Please install Node.js manually using one of these methods:"
        echo "1. NodeSource: https://github.com/nodesource/distributions"
        echo "2. nvm: https://github.com/nvm-sh/nvm"
        exit 1
    }
else
    install_user_local || exit 1
fi

echo ""
echo "Node.js installed successfully!"
echo ""

# Verify installation
echo "Verifying installation..."
sleep 1

if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version)
    NPM_VERSION=$(npm --version)

    echo "✓ Node.js: $NODE_VERSION"
    echo "✓ npm: $NPM_VERSION"
else
    echo "✗ Verification failed. Please check installation logs."
    exit 1
fi

echo ""
echo "=== Installation Complete ==="
echo "Next steps:"
echo "1. Run: node --version"
echo "2. Run: npm --version"
echo "3. Configure npm global directory (optional):"
echo "   npm config set prefix ~/.npm-global"
echo "   export PATH=~/.npm-global/bin:\$PATH"
