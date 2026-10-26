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
            if ! curl -fsSL https://deb.nodesource.com/setup_lts.x | $SUDO -E bash -; then
                echo "Failed to add NodeSource repository." >&2
                return 1
            fi
            if ! $SUDO apt-get install -y nodejs; then
                echo "Failed to install nodejs via apt." >&2
                return 1
            fi
            ;;

        fedora|rhel|centos)
            echo "Installing Node.js on Fedora/RHEL/CentOS..."
            if ! curl -fsSL https://rpm.nodesource.com/setup_lts.x | $SUDO bash -; then
                echo "Failed to add NodeSource repository." >&2
                return 1
            fi
            if ! $SUDO dnf install -y nodejs; then
                echo "Failed to install nodejs via dnf." >&2
                return 1
            fi
            ;;

        arch|manjaro)
            echo "Installing Node.js on Arch Linux..."
            if ! $SUDO pacman -S --noconfirm nodejs npm; then
                echo "Failed to install nodejs via pacman." >&2
                return 1
            fi
            ;;

        opensuse*)
            echo "Installing Node.js on openSUSE..."
            if ! $SUDO zypper install -y nodejs npm; then
                echo "Failed to install nodejs via zypper." >&2
                return 1
            fi
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

    if ! fetch "https://nodejs.org/dist/$NODE_VERSION/$TARBALL" > "$TMP_DIR/$TARBALL"; then
        echo "Failed to download $TARBALL." >&2
        return 1
    fi

    # Checksum verification with fallback (sha256sum → shasum → openssl → fail closed)
    verify_checksum() {
        local file="$1" expected="$2"
        if command -v sha256sum &> /dev/null; then
            echo "$expected  $file" | sha256sum -c -s && return 0
        elif command -v shasum &> /dev/null; then
            echo "$expected  $file" | shasum -a 256 -c -s && return 0
        elif command -v openssl &> /dev/null; then
            local actual
            actual=$(openssl dgst -sha256 "$file" | awk '{print $NF}')
            [ "$actual" = "$expected" ] && return 0
        else
            echo "No checksum tool available (sha256sum/shasum/openssl). Aborting." >&2
            return 1
        fi
    }

    if command -v sha256sum &> /dev/null || command -v shasum &> /dev/null || command -v openssl &> /dev/null; then
        EXPECTED=$(fetch "https://nodejs.org/dist/$NODE_VERSION/SHASUMS256.txt" | grep " $TARBALL\$" | awk '{print $1}')
        if [ -z "$EXPECTED" ]; then
            echo "Could not find checksum for $TARBALL in SHASUMS256.txt." >&2
            return 1
        fi
        if ! verify_checksum "$TMP_DIR/$TARBALL" "$EXPECTED"; then
            echo "Checksum verification failed for $TARBALL." >&2
            return 1
        fi
        echo "Checksum verified."
    else
        echo "No checksum tool available — skipping verification (aborted instead of silently skipping)." >&2
        return 1
    fi

    rm -rf "$DEST"
    mkdir -p "$DEST" "$PREFIX/bin"
    if ! tar -xzf "$TMP_DIR/$TARBALL" -C "$DEST" --strip-components=1; then
        echo "Failed to extract $TARBALL." >&2
        return 1
    fi
    for bin in node npm npx corepack; do
        if [ -e "$DEST/bin/$bin" ]; then
            ln -sfn "$DEST/bin/$bin" "$PREFIX/bin/$bin"
        fi
    done

    # Try to symlink to /usr/local/bin for non-interactive shell support
    if [ -w "/usr/local/bin" ] || [ "$(id -u)" -eq 0 ]; then
        for bin in node npm npx corepack; do
            if [ -e "$PREFIX/bin/$bin" ]; then
                ln -sfn "$PREFIX/bin/$bin" "/usr/local/bin/$bin" 2>/dev/null || true
            fi
        done
    fi

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
    # This path is reached only when install_with_system_pm returned non-zero.
    # Distinguish "unsupported OS" from "install failure" by checking if
    # the case statement had a matching branch.
    case $OS in
        ubuntu|debian|fedora|rhel|centos|arch|manjaro|opensuse*)
            echo "System package manager install failed for ${OS}. Falling back to user-local install."
            install_user_local || {
                echo "User-local install also failed. Please install Node.js manually." >&2
                echo "1. NodeSource: https://github.com/nodesource/distributions"
                echo "2. nvm: https://github.com/nvm-sh/nvm"
                exit 1
            }
            ;;
        *)
            echo "No system package manager recipe for: ${OS:-unknown}; falling back to user-local install."
            install_user_local || exit 1
            ;;
    esac
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
