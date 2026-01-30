# Offline Installation Guide

## Overview

Install Python, Node.js, and uv without internet connectivity.

## Preparation (Online Machine)

### Download Python

**Windows:**
1. Visit python.org/downloads
2. Download "Windows installer (64-bit)"
3. Save file: `python-3.11.x-amd64.exe`

**Linux:**
```bash
# Download source tarball
wget https://www.python.org/ftp/python/3.11.x/Python-3.11.x.tgz

# Or use your distro's package cache
apt-get download python3 python3-pip
dnf download python3 python3-pip
```

**macOS:**
```bash
# Download official installer
curl -O https://www.python.org/ftp/python/3.11.x/python-3.11.x-macos11.pkg
```

### Download Node.js

**Windows:**
1. Visit nodejs.org/download
2. Download "Windows Installer (.msi)" - 64-bit
3. Save file: `node-v20.x.x-x64.msi`

**Linux:**
```bash
# Download binary tarball
wget https://nodejs.org/dist/v20.x.x/node-v20.x.x-linux-x64.tar.xz
```

**macOS:**
```bash
# Download PKG installer
curl -O https://nodejs.org/dist/v20.x.x/node-v20.x.x.pkg
```

### Download uv

**All Platforms:**
```bash
# Download prebuilt binary
# Windows
curl -LO https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip

# Linux
curl -LO https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-unknown-linux-gnu.tar.gz

# macOS (Intel)
curl -LO https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-apple-darwin.tar.gz

# macOS (Apple Silicon)
curl -LO https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-apple-darwin.tar.gz
```

### Download Python Packages

```bash
# Create requirements.txt with needed packages
cat > requirements.txt <<EOF
fastmcp
anthropic
openai
requests
EOF

# Download packages and dependencies
pip download -r requirements.txt -d ./packages/
```

## Installation (Offline Machine)

### Python Installation

**Windows:**
```powershell
# Run installer
.\python-3.11.x-amd64.exe /quiet InstallAllUsers=1 PrependPath=1
```

**Linux (from binary):**
```bash
# Extract and install
tar xzf Python-3.11.x.tgz
cd Python-3.11.x
./configure --prefix=/usr/local
make
sudo make install
```

**Linux (from .deb packages):**
```bash
sudo dpkg -i python3_*.deb python3-pip_*.deb
sudo apt-get install -f  # Fix dependencies if needed
```

**macOS:**
```bash
# Install PKG
sudo installer -pkg python-3.11.x-macos11.pkg -target /
```

### Node.js Installation

**Windows:**
```powershell
# Run MSI installer
msiexec /i node-v20.x.x-x64.msi /quiet
```

**Linux:**
```bash
# Extract binary
tar xJf node-v20.x.x-linux-x64.tar.xz
sudo mv node-v20.x.x-linux-x64 /opt/nodejs

# Add to PATH
echo 'export PATH="/opt/nodejs/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

**macOS:**
```bash
# Install PKG
sudo installer -pkg node-v20.x.x.pkg -target /
```

### uv Installation

**Windows:**
```powershell
# Extract and move to PATH
Expand-Archive uv-x86_64-pc-windows-msvc.zip
Move-Item uv-x86_64-pc-windows-msvc\uv.exe $env:USERPROFILE\.cargo\bin\
```

**Linux/macOS:**
```bash
# Extract and install
tar xzf uv-*.tar.gz
sudo mv uv /usr/local/bin/
chmod +x /usr/local/bin/uv
```

### Python Packages Installation

```bash
# Install from downloaded packages
pip install --no-index --find-links=./packages/ -r requirements.txt
```

## Verification

```bash
# Check installations
python --version
pip --version
node --version
npm --version
uv --version

# Test package import
python -c "import fastmcp; print('fastmcp OK')"
```

## Creating Offline Package Cache

### For Python Projects

```bash
# Create wheel cache
pip wheel -r requirements.txt -w ./wheelhouse/

# Install from wheelhouse
pip install --no-index --find-links=./wheelhouse/ -r requirements.txt
```

### For Node.js Projects

```bash
# Create offline npm cache
npm install --cache ./npm-cache/ --prefer-offline

# Or use npm pack
npm pack package-name
```

### For uv

```bash
# uv caches packages automatically in ~/.cache/uv/
# Copy cache to offline machine:
tar czf uv-cache.tar.gz ~/.cache/uv/

# On offline machine:
tar xzf uv-cache.tar.gz -C ~/
```

## Portable Installation

### Python Portable (Windows)

1. Download "embeddable package" from python.org
2. Extract to `C:\Python311Portable\`
3. Add `python311._pth` file:
   ```
   python311.zip
   .
   ./Lib/site-packages
   ```
4. Install pip manually:
   ```powershell
   python get-pip.py --no-index
   ```

### Node.js Portable (Windows)

1. Download ZIP archive from nodejs.org
2. Extract to desired location
3. Add to PATH or use full path: `C:\node\node.exe`

## Corporate Environment

### Using Internal Mirrors

```bash
# Python (pip)
pip install --index-url http://internal-pypi/simple/ package-name

# Node.js (npm)
npm config set registry http://internal-npm/

# uv
export UV_INDEX_URL=http://internal-pypi/simple/
```

### Proxy Configuration

```bash
# System-wide proxy
export HTTP_PROXY=http://proxy:8080
export HTTPS_PROXY=http://proxy:8080

# pip proxy
pip install --proxy http://proxy:8080 package-name

# npm proxy
npm config set proxy http://proxy:8080
npm config set https-proxy http://proxy:8080
```

## Troubleshooting

### Missing System Libraries (Linux)

Python compilation may need:
```bash
# Debian/Ubuntu
sudo apt-get install build-essential libssl-dev libffi-dev python3-dev

# Fedora/RHEL
sudo dnf groupinstall "Development Tools"
sudo dnf install openssl-devel libffi-devel python3-devel
```

### Certificate Issues

```bash
# Disable SSL verification (testing only)
pip install --trusted-host pypi.org --trusted-host files.pythonhosted.org package-name

# Or add custom CA certificate
export REQUESTS_CA_BUNDLE=/path/to/ca-bundle.crt
```

### Permission Errors

```bash
# Install to user directory (no sudo needed)
pip install --user package-name

# Or use virtual environment
python -m venv venv
source venv/bin/activate
pip install package-name
```
