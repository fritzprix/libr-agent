#!/bin/bash
# Python Installation Script for Linux
# Non-interactive. Order: uv-managed Python (user scope), then the system package
# manager only as root or with passwordless sudo (sudo -n). Never waits for a password.

set -u

VERSION_GIVEN=0
[ $# -ge 1 ] && VERSION_GIVEN=1
VERSION="${1:-3.12}"
FORCE="${2:-}"

echo "=== Python Installation Script ==="
echo "Target Version: Python $VERSION"
echo ""

# With an explicit version, only that version counts (python3.9, or uv's 3.9).
# With no argument, any working python3 is enough.
has_requested_python() {
    if [ "$VERSION_GIVEN" -eq 1 ]; then
        if command -v "python$VERSION" > /dev/null 2>&1 && "python$VERSION" --version > /dev/null 2>&1; then
            return 0
        fi
        command -v uv > /dev/null 2>&1 && uv python find "$VERSION" > /dev/null 2>&1
        return
    fi
    command -v python3 > /dev/null 2>&1 && python3 --version > /dev/null 2>&1
}

if [ "$FORCE" != "--force" ] && has_requested_python; then
    if [ "$VERSION_GIVEN" -eq 1 ]; then
        echo "Python $VERSION already available."
        echo "Run scripts with: uv run --python $VERSION <script.py> (or python$VERSION)"
    else
        echo "Python already installed: $(python3 --version)"
    fi
    echo "Nothing to do. Pass '$VERSION --force' to install Python $VERSION anyway."
    exit 0
fi

# 1. uv-managed Python: user scope, no root needed. Run with `uv run --python`.
if command -v uv > /dev/null 2>&1; then
    echo "Installing Python $VERSION with uv..."
    if uv python install "$VERSION" && FOUND=$(uv python find "$VERSION"); then
        echo "✓ Python $VERSION (uv): $FOUND"
        echo "Run scripts with: uv run --python $VERSION <script.py>"
        exit 0
    fi
    echo "uv python install failed."
fi

# 2. System package manager: only as root or with passwordless sudo.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo > /dev/null 2>&1 && sudo -n true 2> /dev/null; then
        SUDO="sudo -n"
    else
        echo "✗ uv is missing or failed, and there is no root or passwordless sudo, so Python cannot be installed here."
        echo "NEXT: Report this to the user. Ask them to install uv (install_uv.sh) or Python $VERSION, then run verify_setup.sh again."
        exit 1
    fi
fi

OS=""
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
fi

case $OS in
    ubuntu|debian)
        $SUDO apt-get update && $SUDO apt-get install -y python3 python3-pip python3-venv ;;
    fedora|rhel|centos)
        $SUDO dnf install -y python3 python3-pip ;;
    arch|manjaro)
        $SUDO pacman -S --noconfirm python python-pip ;;
    opensuse*)
        $SUDO zypper install -y python3 python3-pip ;;
    *)
        echo "✗ Unsupported distribution: ${OS:-unknown}"
        echo "NEXT: Report this to the user."
        exit 1 ;;
esac

# The distribution package is its default python3, which may not be the version asked for.
if has_requested_python; then
    echo "✓ Python: $(python3 --version)"
    echo "Next step: run verify_setup.sh to confirm."
    exit 0
fi

if [ "$VERSION_GIVEN" -eq 1 ]; then
    echo "✗ The package manager installed the distribution's python3, not Python $VERSION."
    echo "NEXT: Report this to the user. Python $VERSION needs uv (install_uv.sh, then uv python install $VERSION)."
    exit 1
fi

echo "✗ Python installation did not produce a working python3."
echo "NEXT: Report this to the user."
exit 1
