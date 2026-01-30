#!/bin/bash
# Python Installation Script for Linux

set -e

echo "=== Python Installation Script ==="
echo ""

# Detect distribution
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    OS_VERSION=$VERSION_ID
else
    echo "Cannot detect OS distribution."
    exit 1
fi

echo "Detected OS: $OS $OS_VERSION"
echo ""

# Check if Python is already installed
if command -v python3 &> /dev/null; then
    CURRENT_VERSION=$(python3 --version)
    echo "Python already installed: $CURRENT_VERSION"
    read -p "Do you want to continue with installation? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
fi

# Install Python based on distribution
case $OS in
    ubuntu|debian)
        echo "Installing Python on Ubuntu/Debian..."
        sudo apt-get update
        sudo apt-get install -y python3 python3-pip python3-venv python3-dev
        ;;
    
    fedora|rhel|centos)
        echo "Installing Python on Fedora/RHEL/CentOS..."
        sudo dnf install -y python3 python3-pip python3-devel
        ;;
    
    arch|manjaro)
        echo "Installing Python on Arch Linux..."
        sudo pacman -S --noconfirm python python-pip
        ;;
    
    opensuse*)
        echo "Installing Python on openSUSE..."
        sudo zypper install -y python3 python3-pip python3-devel
        ;;
    
    *)
        echo "Unsupported distribution: $OS"
        echo "Please install Python manually from python.org"
        exit 1
        ;;
esac

echo ""
echo "Python installed successfully!"
echo ""

# Verify installation
echo "Verifying installation..."
sleep 1

if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version)
    PIP_VERSION=$(pip3 --version)
    
    echo "✓ Python: $PYTHON_VERSION"
    echo "✓ pip: $PIP_VERSION"
else
    echo "✗ Verification failed. Please check installation logs."
    exit 1
fi

# Upgrade pip
echo ""
echo "Upgrading pip to latest version..."
python3 -m pip install --upgrade pip --user

echo ""
echo "=== Installation Complete ==="
echo "Next steps:"
echo "1. Run: python3 --version"
echo "2. Install uv: pip3 install uv --user"
echo "3. Add ~/.local/bin to PATH if not already present"
