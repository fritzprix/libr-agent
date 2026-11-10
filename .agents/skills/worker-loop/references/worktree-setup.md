# Dedicated Worktree Setup

All edits, builds, commits, and pushes happen in a dedicated, persistent worktree per worker — **never in the user's checkout**, which keeps its branch and uncommitted work untouched. Skill files, scripts, and ignored rule files such as `.agents/AGENTS.md` are read from the main checkout (`$MAIN`). The worktree directory name is the worker slot (`worker`, `worker-2`, …). After `tick_gate.py --lock`, use the printed `WORKER <name>`. Whoever holds the slot owns its worktree.

## Setup

- **Bash (Linux/macOS)**:
  ```bash
  MAIN=$(git rev-parse --show-toplevel)       # the user's checkout
  WORKER="${WORKER_ID:?set WORKER_ID from tick_gate WORKER line or export before starting}"
  WT="$MAIN/../$(basename "$MAIN").worktrees/$WORKER"
  BASE=$(git branch -r --list "origin/dev/*" --sort=-v:refname | head -n 1 | sed 's/^[ *]*origin\///')
  python3 -c "import subprocess, sys; from pathlib import Path; out = subprocess.run(['git', '-C', sys.argv[1], 'worktree', 'list', '--porcelain'], capture_output=True, text=True).stdout; target = Path(sys.argv[2]).resolve(); sys.exit(0 if any(line.startswith('worktree ') and Path(line[9:].strip()).resolve() == target for line in out.splitlines()) else 1)" "$MAIN" "$WT" \
    || git -C "$MAIN" worktree add --detach "$WT" "origin/$BASE"
  ```
- **PowerShell (Windows)**:
  ```powershell
  $MAIN = git rev-parse --show-toplevel
  if (-not $env:WORKER_ID) { throw "Set WORKER_ID from tick_gate WORKER line before worktree setup." }
  $WORKER = $env:WORKER_ID
  $WT = "$MAIN/../$((Split-Path $MAIN -Leaf)).worktrees/$WORKER"
  $BASE = (git branch -r --list "origin/dev/*" --sort=-v:refname | Select-Object -First 1).Trim().Replace("origin/", "")
  python3 -c "import subprocess, sys; from pathlib import Path; out = subprocess.run(['git', '-C', sys.argv[1], 'worktree', 'list', '--porcelain'], capture_output=True, text=True).stdout; target = Path(sys.argv[2]).resolve(); sys.exit(0 if any(line.startswith('worktree ') and Path(line[9:].strip()).resolve() == target for line in out.splitlines()) else 1)" "$MAIN" "$WT"
  if ($LASTEXITCODE -ne 0) {
      git -C "$MAIN" worktree add --detach "$WT" "origin/$BASE"
  }
  ```

## Worktree Lifecycle

- Keep the worktree persistent between ticks so Rust `target/` and `node_modules` stay warm; the first tick pays a full build. Do not share `target/` with `$MAIN`. Each worktree has its own `src-tauri/target/` (including `target/ci-test` for `pnpm rust:test`); `scripts/run-rust-command.cjs` runs cargo with `cwd` under that worktree, so workers do not share a cargo target dir unless they share the same worktree path. Builds from all worktrees and checkouts share a machine-wide limit (`scripts/rust-build-slot.cjs`, default one per 32GB of RAM, `LIBRAGENT_RUST_MAX_CONCURRENT` overrides it).
- Run `pnpm install --frozen-lockfile` in `$WT` when `pnpm-lock.yaml` differs from the hash stored in state (`lockHash`).
- Ignored files are not carried over.
- A branch can be checked out in only one worktree. Get a PR branch with `worker_worktree.py switch`: it reports `BRANCH BUSY` for a live run or a slot with `current`, detaches an abandoned clean slot, and otherwise (`$MAIN`, other worktrees, a slot with leftover changes) puts you on `origin/<head>` detached and prints the push command.
- Detach at the end of a tick with no `current`, so the branch does not stay occupied.
- Reset only with `worker_worktree.py reset`, which verifies the lease, stashes uncommitted changes as `worker-loop rescue: ...`, and does nothing when `current` is set. Recover with `git stash list` and `git stash apply <sha>`. Never run `reset --hard` or `clean` directly, and never against `$MAIN`.
