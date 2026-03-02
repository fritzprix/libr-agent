#!/bin/bash
# System Setup Verification Script for Linux/macOS
# Checks Python, Node.js, and uv installations

set +e  # Don't exit on errors

echo "========================================"
echo "  System Setup Verification for MCP"
echo "========================================"
echo ""

all_good=true

# Check Python
echo "[1/3] Checking Python..."
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1)
    PIP_VERSION=$(pip3 --version 2>&1)
    
    echo "  ✓ Python: $PYTHON_VERSION"
    echo "  ✓ pip: $PIP_VERSION"
    
    # Check Python version (3.11+)
    if python3 --version 2>&1 | grep -qE "Python 3\.([0-9]|10)\."; then
        echo "  ⚠ Warning: Python 3.11+ recommended"
    fi
else
    echo "  ✗ Python not found"
    echo "    Install with: ./scripts/install_python.sh"
    all_good=false
fi

echo ""

# Check Node.js
echo "[2/3] Checking Node.js..."
if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version 2>&1)
    NPM_VERSION=$(npm --version 2>&1)
    
    echo "  ✓ Node.js: $NODE_VERSION"
    echo "  ✓ npm: $NPM_VERSION"
    
    # Check Node version (18+)
    NODE_MAJOR=$(echo $NODE_VERSION | sed 's/v\([0-9]*\).*/\1/')
    if [ "$NODE_MAJOR" -lt 18 ]; then
        echo "  ⚠ Warning: Node.js 18+ recommended (found v$NODE_MAJOR)"
    fi
else
    echo "  ✗ Node.js not found"
    echo "    Install with: ./scripts/install_node.sh"
    all_good=false
fi

echo ""

# Check uv
echo "[3/3] Checking uv..."
if command -v uv &> /dev/null; then
    UV_VERSION=$(uv --version 2>&1)
    echo "  ✓ uv: $UV_VERSION"
else
    echo "  ✗ uv not found"
    echo "    Install with: ./scripts/install_uv.sh"
    all_good=false
fi

echo ""
echo "========================================"

# Summary
if [ "$all_good" = true ]; then
    echo "✓ All systems ready for MCP!"
    echo ""
    echo "You can now:"
    echo "  • Run Python-based MCP servers"
    echo "  • Run Node.js-based MCP servers"
    echo "  • Use uv for fast Python package management"
    echo ""
else
    echo "✗ Some components are missing"
    echo ""
    echo "Please install missing components:"
    echo "  • Python:  ./scripts/install_python.sh"
    echo "  • Node.js: ./scripts/install_node.sh"
    echo "  • uv:      ./scripts/install_uv.sh"
    echo ""
    echo "Or install all at once:"
    echo "  ./scripts/install_python.sh && ./scripts/install_node.sh && ./scripts/install_uv.sh"
    echo ""
fi

# PATH check
echo "========================================"
echo "PATH Configuration:"
echo ""

# Show relevant PATH entries
echo "$PATH" | tr ':' '\n' | grep -E 'python|node|npm|cargo|\.local' > /tmp/relevant_paths.txt

if [ -s /tmp/relevant_paths.txt ]; then
    echo "Relevant PATH entries:"
    while IFS= read -r path; do
        echo "  • $path"
    done < /tmp/relevant_paths.txt
    rm /tmp/relevant_paths.txt
else
    echo "⚠ No Python/Node paths found in PATH"
    echo "You may need to restart your terminal or source your shell RC file."
fi

echo ""
echo "========================================"

# Exit with appropriate code
if [ "$all_good" = true ]; then
    exit 0
else
    exit 1
fi
