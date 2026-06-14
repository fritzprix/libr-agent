#!/bin/bash
# Node.js Installation Script for Linux

set -e

echo "=== Node.js Installation Script ==="
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

# Check if Node.js is already installed
if command -v node &> /dev/null; then
    CURRENT_VERSION=$(node --version)
    echo "Node.js already installed: $CURRENT_VERSION"
    read -p "Do you want to continue with installation? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
fi

# Install Node.js based on distribution
case $OS in
    ubuntu|debian)
        echo "Installing Node.js on Ubuntu/Debian..."
        curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
        sudo apt-get install -y nodejs
        ;;
    
    fedora|rhel|centos)
        echo "Installing Node.js on Fedora/RHEL/CentOS..."
        curl -fsSL https://rpm.nodesource.com/setup_lts.x | sudo bash -
        sudo dnf install -y nodejs
        ;;
    
    arch|manjaro)
        echo "Installing Node.js on Arch Linux..."
        sudo pacman -S --noconfirm nodejs npm
        ;;
    
    opensuse*)
        echo "Installing Node.js on openSUSE..."
        sudo zypper install -y nodejs npm
        ;;
    
    *)
        echo "Unsupported distribution: $OS"
        echo "Please install Node.js manually using one of these methods:"
        echo "1. NodeSource: https://github.com/nodesource/distributions"
        echo "2. nvm: https://github.com/nvm-sh/nvm"
        exit 1
        ;;
esac

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
