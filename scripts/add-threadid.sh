#!/bin/bash
# Script to add threadId to Message object creations across the codebase
# This adds threadId: messages[0]?.threadId || messages[0]?.sessionId after sessionId: messages[0]?.sessionId patterns

set -e

echo "Adding threadId to Message object creations..."

# Fix use-ai-service.ts - add threadId after sessionId in Message objects
sed -i 's/sessionId: messages\[0\]?\.sessionId,$/&\n            threadId: messages[0]?.threadId || messages[0]?.sessionId,/g' src/hooks/use-ai-service.ts

# Fix use-tool-processor.ts - add threadId after sessionId
sed -i 's/sessionId: messages\[0\]?\.sessionId,$/&\n              threadId: messages[0]?.threadId || messages[0]?.sessionId,/g' src/hooks/use-tool-processor.ts

# Fix test files - add threadId: "test-session-1" after sessionId: "test-session-1"
find src/lib/ai-service/__tests__ -name "*.test.ts" -exec sed -i 's/sessionId: "test-session-\([0-9]\+\)",$/&\n      threadId: "test-session-\1",/g' {} \;

echo "ThreadId additions complete!"
echo "Run 'pnpm exec tsc --noEmit' to verify TypeScript compilation"
