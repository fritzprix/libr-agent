#!/bin/bash

# Check for direct dbService usage outside of service layer
# We exclude:
# - lib/services: The service layer itself
# - lib/db: The DB layer itself
# - test/__tests__: Test files
# - web-mcp: Backend modules (out of scope for React refactoring, though ideally should be fixed too)

RESULT=$(grep -r "dbService\." src/ \
  --include="*.ts" \
  --include="*.tsx" \
  --exclude-dir="services" \
  --exclude-dir="db" \
  --exclude-dir="__tests__" \
  --exclude-dir="test" \
  --exclude-dir="web-mcp" \
  -n)

if [ -n "$RESULT" ]; then
  echo "❌ Direct dbService access found outside service layer:"
  echo "$RESULT"
  exit 1
else
  echo "✅ No direct dbService access found (excluding web-mcp)"
  exit 0
fi
