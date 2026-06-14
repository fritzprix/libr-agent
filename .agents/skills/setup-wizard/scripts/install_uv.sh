#!/bin/bash
# uv Installation Script for Linux/macOS

set -e

echo "=== uv Installation Script ==="
echo ""

# Check if uv is already installed
if command -v uv &> /dev/null; then
    CURRENT_VERSION=$(uv --version)
    echo "uv already installed: $CURRENT_VERSION"
    read -p "Do you want to continue with installation? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
fi

# Method 1: Using standalone installer (recommended)
echo "Installing uv using standalone installer..."
if curl -LsSf https://astral.sh/uv/install.sh | sh; then
    echo "✓ uv installed via standalone installer!"
else
    echo "Standalone installation failed. Trying pip method..."
    
    # Method 2: Using pip
    if ! command -v python3 &> /dev/null; then
        echo "✗ Python not found. Please install Python first."
        echo "Run: ./install_python.sh"
        exit 1
    fi
    
    echo "Installing uv using pip..."
    python3 -m pip install uv --user
    echo "✓ uv installed via pip!"
fi

# Add cargo bin to PATH if not present
CARGO_BIN="$HOME/.cargo/bin"
if [[ ":$PATH:" != *":$CARGO_BIN:"* ]]; then
    echo ""
    echo "Adding $CARGO_BIN to PATH..."
    
    # Detect shell
    if [ -n "$ZSH_VERSION" ]; then
        SHELL_RC="$HOME/.zshrc"
    elif [ -n "$BASH_VERSION" ]; then
        SHELL_RC="$HOME/.bashrc"
    else
        SHELL_RC="$HOME/.profile"
    fi
    
    echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$SHELL_RC"
    export PATH="$HOME/.cargo/bin:$PATH"
    echo "✓ PATH updated in $SHELL_RC!"
fi

# Verify installation
echo ""
echo "Verifying installation..."
sleep 1

# Source the updated PATH
export PATH="$HOME/.cargo/bin:$PATH"

if command -v uv &> /dev/null; then
    UV_VERSION=$(uv --version)
    UV_PATH=$(which uv)
    
    echo "✓ uv: $UV_VERSION"
    echo "  Location: $UV_PATH"
    
    echo ""
    echo "uv installation completed successfully!"
else
    echo "✗ Verification failed. Please run: source $SHELL_RC"
    echo "Then try: uv --version"
    exit 1
fi

echo ""
echo "=== Installation Complete ==="
echo "Next steps:"
echo "1. Run: source $SHELL_RC  (or restart terminal)"
echo "2. Run: uv --version"
echo "3. Create venv: uv venv"
echo "4. Install packages: uv pip install package-name"
