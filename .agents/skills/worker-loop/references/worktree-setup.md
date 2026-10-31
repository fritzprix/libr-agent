# Dedicated Worktree Setup

All edits, builds, commits, and pushes happen in a dedicated, persistent worktree — **never in the user's checkout**, which keeps its branch and uncommitted work untouched. Skill files, scripts, the state file, and ignored rule files such as `.agents/AGENTS.md` are read from the main checkout (`$MAIN`).

## Setup

- **Bash (Linux/macOS)**:
  ```bash
  MAIN=$(git rev-parse --show-toplevel)       # the user's checkout
  WT="$MAIN/../$(basename "$MAIN").worktrees/worker"
  BASE=$(git branch -r --list "origin/dev/*" --sort=-v:refname | head -n 1 | sed 's/^[ *]*origin\///')
  python3 -c "import subprocess, sys, os; out = subprocess.run(['git', '-C', sys.argv[1], 'worktree', 'list', '--porcelain'], capture_output=True, text=True).stdout; target = os.path.abspath(sys.argv[2]); sys.exit(0 if any(line == f'worktree {target}' for line in out.splitlines()) else 1)" "$MAIN" "$WT" \
    || git -C "$MAIN" worktree add --detach "$WT" "origin/$BASE"
  ```
- **PowerShell (Windows)**:
  ```powershell
  $MAIN = git rev-parse --show-toplevel
  $WT = "$MAIN/../$((Split-Path $MAIN -Leaf)).worktrees/worker"
  $BASE = (git branch -r --list "origin/dev/*" --sort=-v:refname | Select-Object -First 1).Trim().Replace("origin/", "")
  python3 -c "import subprocess, sys, os; out = subprocess.run(['git', '-C', sys.argv[1], 'worktree', 'list', '--porcelain'], capture_output=True, text=True).stdout; target = os.path.abspath(sys.argv[2]); sys.exit(0 if any(line == f'worktree {target}' for line in out.splitlines()) else 1)" $MAIN $WT
  if ($LASTEXITCODE -ne 0) {
      git -C $MAIN worktree add --detach "$WT" "origin/$BASE"
  }
  ```

## Worktree Lifecycle

- Keep the worktree persistent between ticks so Rust `target/` and `node_modules` stay warm; the first tick pays a full build. Do not share `target/` with `$MAIN`.
- Run `pnpm install --frozen-lockfile` in `$WT` when `pnpm-lock.yaml` differs from the hash stored in state (`lockHash`).
- Ignored files are not carried over.
- A branch checked out in `$MAIN` cannot be checked out in `$WT`. Then work detached on `origin/<head>` and push with `git push origin HEAD:refs/heads/<head>`.
- `$WT` belongs to the worker: at the start of a tick with no `current`, `git -C "$WT" reset --hard && git -C "$WT" clean -fd` is allowed. Never run these against `$MAIN`.
