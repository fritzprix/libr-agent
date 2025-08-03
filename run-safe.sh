#!/bin/bash

echo "🚀 Starting SynapticFlow with WebKit compatibility settings..."

# Set WebKit environment variables for better compatibility
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_SANDBOX=1
export WEBKIT_FORCE_SANDBOX=0

# Set Wayland/X11 compatibility
export GDK_BACKEND=x11
export MOZ_ENABLE_WAYLAND=0

# Set graphics compatibility
export LIBGL_ALWAYS_SOFTWARE=1
export MESA_GL_VERSION_OVERRIDE=3.3

# Check if we're in a graphical environment
if [ -z "$DISPLAY" ] && [ -z "$WAYLAND_DISPLAY" ]; then
    echo "❌ No display detected. Please run in a graphical environment."
    exit 1
fi

# Check if required packages are installed
if ! dpkg -l | grep -q libwebkit2gtk-4.1-dev; then
    echo "⚠️  WebKit development package not found. Installing..."
    sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev
fi

echo "✅ Environment configured. Starting application..."

# Run the application based on the environment
if [ "$1" = "dev" ]; then
    echo "🔧 Starting in development mode..."
    pnpm tauri dev
elif [ "$1" = "build" ]; then
    echo "🏗️  Building application..."
    pnpm tauri build
else
    echo "Usage: $0 [dev|build]"
    echo "  dev   - Start in development mode"
    echo "  build - Build the application"
    exit 1
fi
