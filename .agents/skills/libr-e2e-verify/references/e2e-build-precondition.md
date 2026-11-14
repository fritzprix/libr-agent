# E2E build precondition

E2E runs against a debug build of a known commit. The harness builds and starts that binary before any checklist or matrix item.

## Build and start

1. Find the merge commit of each PR under test: `gh pr view <N> --json mergeCommit`.
2. Check out a branch head that contains every merge commit. Use a dedicated worktree.
3. Build debug from that checkout: `pnpm tauri build --debug` or the project’s documented debug build command.
4. Start the binary with `LIBRAGENT_E2E_CONTROL=1` and an isolated `LIBRAGENT_DATA_DIR`.

## Read provenance

5. Read `buildSha` and `buildDirty` from `GET /api/e2e-control/status` (capability header) or from `GET /api/health`.
6. For each PR, run `git merge-base --is-ancestor <merge-commit> <buildSha>`. A PR not contained is `blocked: not in build`.
7. Record `buildSha` in every results comment and report.

## Containment helper

```bash
python3 .agents/skills/libr-e2e-verify/scripts/check_pr_build_containment.py \
  --prs 2731,2740 --status-url http://127.0.0.1:3030/api/e2e-control/status \
  --e2e-cap "$(cat "$LIBRAGENT_DATA_DIR/e2e_control.cap")" \
  --repo /path/to/repo
```

Or pass `--build-sha` when the app is not running.
