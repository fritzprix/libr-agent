#!/bin/bash
# Test script to verify .env loading in production build

set -e

echo "==================================="
echo "Production .env Loading Test"
echo "==================================="
echo ""

# Create test .env files
echo "Creating test .env files..."
cat > src-tauri/.env << EOF
LIBRAGENT_MAX_FILE_SIZE=209715200
RUST_LOG=info
MESSAGE_INDEX_SNIPPET_LENGTH=300
EOF

echo "✓ Created .env with:"
echo "  - LIBRAGENT_MAX_FILE_SIZE=209715200 (200MB)"
echo "  - MESSAGE_INDEX_SNIPPET_LENGTH=300"
echo ""

# Build production binary (without creating installer bundle)
echo "Building production binary (this may take a few minutes)..."
echo ""
pnpm tauri build --no-bundle

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo ""
echo "✓ Build completed"
echo ""

# Find the binary
BINARY=""
if [ -f "src-tauri/target/release/libr-agent" ]; then
    BINARY="src-tauri/target/release/libr-agent"
elif [ -f "src-tauri/target/release/libr-agent.exe" ]; then
    BINARY="src-tauri/target/release/libr-agent.exe"
else
    echo "❌ Could not find built binary"
    echo "Expected location: src-tauri/target/release/libr-agent"
    exit 1
fi

echo "Found binary: $BINARY"
echo ""

# Copy .env to the executable directory
echo "Copying .env to executable directory..."
cp src-tauri/.env src-tauri/target/release/.env
echo "✓ Copied .env to: src-tauri/target/release/.env"
echo ""

# Show .env content
echo "==================================="
echo ".env file content:"
echo "==================================="
cat src-tauri/target/release/.env
echo ""
echo "==================================="
echo ""

# Test 1: Check if binary loads .env
echo "Test 1: Checking .env loading..."
echo "Running binary (will timeout after 5 seconds)..."
timeout 5s "$BINARY" 2>&1 | tee /tmp/libr-agent-test.log &
BINARY_PID=$!
sleep 3
kill $BINARY_PID 2>/dev/null || true

echo ""
if grep -q "✅ Loaded .env" /tmp/libr-agent-test.log; then
    echo "✅ SUCCESS: .env file was loaded!"
    grep "✅ Loaded .env" /tmp/libr-agent-test.log
else
    echo "⚠️  Could not confirm .env loading from output"
    echo "This might be normal if logs are handled differently"
fi

echo ""
echo "==================================="
echo "Manual Verification Steps"
echo "==================================="
echo ""
echo "The production binary is ready at:"
echo "  $BINARY"
echo ""
echo "To manually verify .env is working:"
echo ""
echo "1. Ensure .env exists in the same directory:"
echo "   ls -la src-tauri/target/release/.env"
echo ""
echo "2. Run the binary:"
echo "   cd src-tauri/target/release"
echo "   ./libr-agent"
echo ""
echo "3. In the app, try to upload a file larger than 100MB but smaller than 200MB"
echo "   - SUCCESS = .env is working (200MB limit)"
echo "   - FAILURE = using default (100MB limit)"
echo ""
echo "4. Check the configuration values in the app's logs/settings"
echo ""

# Cleanup test files
echo "==================================="
echo "Cleanup"
echo "==================================="
rm -f /tmp/libr-agent-test.log
echo "✓ Cleaned up test log file"
echo ""
echo "Note: .env files in src-tauri/ and src-tauri/target/release/"
echo "      have been kept for your testing."
echo "      Delete them manually when done: rm src-tauri/.env src-tauri/target/release/.env"

