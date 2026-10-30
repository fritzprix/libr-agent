#!/bin/bash
# Oracle reference only: the reply is graded after a real wake (see README).
set -euo pipefail
/app/sms_monitor subscribe --hook 'curl -sf -X POST "$LIBRAGENT_WAKE_URL" -H "Content-Type: application/json" -d "{\"event\":\"sms\",\"messageId\":\"$SMS_MESSAGE_ID\"}"'
