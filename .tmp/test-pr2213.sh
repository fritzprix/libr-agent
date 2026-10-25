#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
git checkout -b pr-2213-test origin/refactor/service-proxy-manager-tests-2186 2>/dev/null
pnpm rust:test --test service_proxy_manager 2>&1 | tee /home/fritzprix/my_works/libr-agent/.tmp/pr2213-test-output.txt
echo "EXIT=$?"
git checkout - 2>/dev/null
git branch -D pr-2213-test 2>/dev/null
