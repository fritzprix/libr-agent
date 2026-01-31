#!/usr/bin/env bash

# Usage: ./dump_error.sh [LINES]
# Default: 20 lines
LINES="${1:-20}"

echo "🔍 Extracting error logs..."
echo "📊 Lines to extract: $LINES"

if ./scripts/dump_log.sh "$LINES"; then
    echo "✅ Log extraction completed"
    echo "🔧 Processing error context..."
    ./scripts/extract_error_context.sh > error.txt
    echo "✅ Error context saved to error.txt"
    echo "📄 Total lines in error.txt: $(wc -l < error.txt)"
else
    echo "❌ Log extraction failed"
    exit 1
fi
