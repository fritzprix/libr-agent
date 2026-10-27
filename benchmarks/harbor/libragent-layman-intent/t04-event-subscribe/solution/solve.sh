#!/bin/bash
set -euo pipefail
/app/sms_monitor subscribe --hook 'curl -s -X POST "$LIBRAGENT_WAKE_URL" -H "Content-Type: application/json" -d "{\"event\":\"sms\"}"'
