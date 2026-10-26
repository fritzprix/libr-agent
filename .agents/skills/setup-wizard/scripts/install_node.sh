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

# The user-local fallback downloads Linux tarballs, so refuse other kernels instead of
# installing binaries that cannot run (macOS: use Homebrew or the nodejs.org installer).
if [ "$(uname -s)" != "Linux" ]; then
    echo "This script supports Linux only (detected $(uname -s))." >&2
    echo "On macOS, run 'brew install node' or use the installer from https://nodejs.org." >&2
    exit 1
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

install_with_system_pm() {
    case $OS in
        ubuntu|debian)
            echo "Installing Node.js on Ubuntu/Debian..."
            # Download first: a pipeline's status is bash's, so `curl -f | bash` hides a failed download.
            if ! setup_script=$(fetch https://deb.nodesource.com/setup_lts.x) \
                || ! printf '%s\n' "$setup_script" | $SUDO -E bash -; then
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
            if ! setup_script=$(fetch https://rpm.nodesource.com/setup_lts.x) \
                || ! printf '%s\n' "$setup_script" | $SUDO bash -; then
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
            return 2  # no recipe for this OS (distinct from a failed install)
            ;;
    esac
}

# Link $1/bin/{node,npm,npx,corepack} into $2 so non-interactive shells find them.
# Never replaces an entry that another install provides. Returns 1 if any link was skipped.
link_system_bin() {
    local prefix="$1" sysbin="$2" bin target status=0
    for bin in node npm npx corepack; do
        [ -e "$prefix/bin/$bin" ] || continue
        target="$sysbin/$bin"
        if [ -e "$target" ] || [ -L "$target" ]; then
            case $(readlink "$target" 2> /dev/null) in
                "$prefix"/*) ;;
                *) status=1; continue ;;
            esac
        fi
        ln -sfn "$prefix/bin/$bin" "$target" 2> /dev/null || status=1
    done
    return $status
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
        echo "No checksum tool available (sha256sum/shasum/openssl). Aborting." >&2
        return 1
    fi

    rm -rf "$DEST"
    if ! mkdir -p "$DEST" "$PREFIX/bin"; then
        echo "Failed to create $DEST." >&2
        return 1
    fi
    if ! tar -xzf "$TMP_DIR/$TARBALL" -C "$DEST" --strip-components=1; then
        echo "Failed to extract $TARBALL." >&2
        return 1
    fi
    for bin in node npm npx corepack; do
        if [ -e "$DEST/bin/$bin" ] && ! ln -sfn "$DEST/bin/$bin" "$PREFIX/bin/$bin"; then
            echo "Failed to link $bin into $PREFIX/bin." >&2
            return 1
        fi
    done

    # Non-interactive shells (bash -c) skip the rc files, so link into /usr/local/bin when possible.
    SYSTEM_BIN_DIR="${LIBRAGENT_SYSTEM_BIN_DIR:-/usr/local/bin}"
    SYSTEM_LINKED=1
    link_system_bin "$PREFIX" "$SYSTEM_BIN_DIR" || SYSTEM_LINKED=0

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
    if [ "$SYSTEM_LINKED" -eq 0 ]; then
        echo ""
        if [ -e "$SYSTEM_BIN_DIR/node" ]; then
            echo "$SYSTEM_BIN_DIR/node already exists ($(readlink -f "$SYSTEM_BIN_DIR/node")) and was left alone."
            echo "Non-interactive shells (e.g. 'bash -c \"node -v\"') will use that one, not this install."
        else
            echo "Could not link into $SYSTEM_BIN_DIR (not writable), so non-interactive shells"
            echo "(e.g. 'bash -c \"node -v\"') will not find node on PATH."
        fi
        echo "To use this install there, use the absolute path:"
        echo "  $PREFIX/bin/node"
        echo "or run: export PATH=\"$PREFIX/bin:\$PATH\""
    fi
}

if [ "$USE_SYSTEM_PM" -eq 1 ]; then
    rc=0
    install_with_system_pm || rc=$?
    if [ "$rc" -eq 2 ]; then
        echo "No system package manager recipe for: ${OS:-unknown}; falling back to user-local install."
    elif [ "$rc" -ne 0 ]; then
        echo "System package manager install failed for ${OS}. Falling back to user-local install."
    fi
    if [ "$rc" -ne 0 ]; then
        install_user_local || {
            echo "User-local install also failed. Please install Node.js manually." >&2
            echo "1. NodeSource: https://github.com/nodesource/distributions"
            echo "2. nvm: https://github.com/nvm-sh/nvm"
            exit 1
        }
    fi
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
