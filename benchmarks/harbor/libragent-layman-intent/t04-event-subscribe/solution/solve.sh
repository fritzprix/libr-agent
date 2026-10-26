#!/bin/bash
set -euo pipefail
/app/relayctl subscribe --topic release.gate.passed --once --exec 'curl -s -X POST "$LIBRAGENT_WAKE_URL" -H "Content-Type: application/json" -d "{\"event\":\"release.gate.passed\"}"'
