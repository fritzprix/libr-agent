# liblet CLI interface: benchmark and proposal

Status: proposal for #2699 (part of epic #2698). Nothing here is implemented. The TUI (#2702) and headless (#2703) tickets should build against section 3 once the open decisions in section 5 are answered.

The invocation forms used in the epic discussion (`liblet`, `liblet "…"`, `liblet --prompt "…"`, `--hook`) were examples of intent. This document is the contract proposal; those examples are not.

## 1. Sources

Docs were read on 2026-11-12. "Observed" means the `--help` output of a locally installed binary. Codex CLI and Aider were not installed, so their rows are docs only.

| Tool        | Sources                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | Docs: [headless](https://code.claude.com/docs/en/headless), [CLI reference](https://code.claude.com/docs/en/cli-reference), [hooks](https://code.claude.com/docs/en/hooks), [permission modes](https://code.claude.com/docs/en/permission-modes). Observed: `claude --help`, version 2.1.322.                                                                                                                                                                                                                                                                           |
| Codex CLI   | Docs: [non-interactive mode](https://developers.openai.com/codex/noninteractive), [CLI reference](https://developers.openai.com/codex/cli/reference), [config reference](https://developers.openai.com/codex/config-reference). These URLs redirect to `learn.chatgpt.com`.                                                                                                                                                                                                                                                                                             |
| Gemini CLI  | Docs: [headless](https://geminicli.com/docs/cli/headless/), [CLI reference](https://geminicli.com/docs/cli/cli-reference/), [hooks](https://geminicli.com/docs/hooks/), [hooks reference](https://geminicli.com/docs/hooks/reference/), [session management](https://geminicli.com/docs/cli/session-management/), [policy engine](https://geminicli.com/docs/reference/policy-engine). Observed: `gemini --help`, version 0.45.2. The docs site carries a banner saying Gemini CLI was replaced by Antigravity CLI on 2026-06-18, so the docs may lag current installs. |
| Aider       | Docs: [scripting](https://aider.chat/docs/scripting.html), [options](https://aider.chat/docs/config/options.html), [notifications](https://aider.chat/docs/usage/notifications.html), [FAQ](https://aider.chat/docs/faq.html).                                                                                                                                                                                                                                                                                                                                          |

Limits of this reading: the Claude Code hooks page, the Codex CLI reference, and the Codex config reference are long, and only the first part of each was read word for word. Facts from the rest are marked "(summary)" below. "Not documented" means the pages read do not state it; it does not mean the tool lacks the behavior.

## 2. Comparison

### 2.1 Interactive vs headless selection

| Tool        | Interactive                                                                  | Headless / one-shot                                                                            | No arguments   |
| ----------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------- |
| Claude Code | `claude`; `claude "query"` starts the TUI with an initial prompt             | `-p` / `--print`: "Print response without interactive mode"                                    | TUI            |
| Codex CLI   | `codex`; `codex "prompt"` pre-fills the TUI (summary)                        | Subcommand `codex exec "prompt"` (alias `codex e`)                                             | TUI            |
| Gemini CLI  | `gemini`; `gemini "query"` in a TTY; `-i` runs a prompt and stays in the TUI | `-p` / `--prompt`. Also triggered automatically "when the CLI is run in a non-TTY environment" | TUI            |
| Aider       | `aider` (chat mode is the implied default)                                   | `--message` / `-m`, `--message-file` / `-f`: sends one message, then exits                     | Chat (implied) |

Stdin for the prompt:

- Claude Code: `-p` reads piped stdin (`cat file | claude -p "query"`), capped at 10 MB.
- Codex CLI: with a prompt argument, piped stdin is extra context. With no prompt, or with `-`, the prompt is read from stdin.
- Gemini CLI: `-p` text is "Appended to stdin input if provided".
- Aider: not documented.

### 2.2 Output

| Tool        | Default                                                    | Machine-readable                                                                                                                                                         |
| ----------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Claude Code | `--output-format text`                                     | `json`: one object with `result`, `session_id`, `total_cost_usd`. `stream-json`: JSONL, last line is a `result` message; denied calls are listed in `permission_denials` |
| Codex CLI   | Progress on stderr, only the final agent message on stdout | `--json`: JSONL events `thread.started`, `turn.started`, `turn.completed`, `turn.failed`, `item.*`, `error` (summary). `-o <path>` writes the final message to a file    |
| Gemini CLI  | `--output-format text`                                     | `json`: one object with `response`, `stats`, optional `error`. `stream-json`: JSONL events `init`, `message`, `tool_use`, `tool_result`, `error`, `result`               |
| Aider       | Colorized text; `--no-pretty`, `--no-stream` tone it down  | None documented                                                                                                                                                          |

Claude Code and Gemini CLI use the same flag name and the same three values. No tool documents a dedicated quiet flag; the Codex stdout/stderr split does that job.

### 2.3 Sessions and working directory

| Tool        | Resume                                                                                                     | Scope and working directory                                                                                                                                           |
| ----------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | `-c` / `--continue` (most recent), `-r` / `--resume <id or name>`, `--session-id <uuid>`, `--fork-session` | `--continue` is scoped to the current directory. `--resume <id>` finds the session "in any project on this machine"                                                   |
| Codex CLI   | `codex resume`, `codex exec resume [SESSION_ID]`, `--last`, `--all` (summary)                              | `--last` uses the current working directory; `--all` widens it. `--cd` / `-C` sets the working directory. `exec` checks for a Git repo unless `--skip-git-repo-check` |
| Gemini CLI  | `-r` / `--resume` with `latest`, an index, or a UUID; `--list-sessions`; `--delete-session`                | "Sessions are project-specific", stored under a hash of the project root                                                                                              |
| Aider       | `--restore-chat-history` (default off); history in `.aider.chat.history.md`                                | Looks for a Git repo by default; how the root is chosen from a subdirectory is not documented                                                                         |

Three of four tools scope "most recent" to the current directory and let an explicit id reach further. That matches the epic: a new session uses the current directory, and a resumed session keeps its own path.

### 2.4 Tool approval in headless mode

| Tool        | Default when a call needs approval and nobody can answer                                                                                 | Documented opt-in to skip                                                                                         |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Claude Code | The call "is denied unless something approves it, such as a `PermissionRequest` hook or a `--permission-prompt-tool`"; the run continues | `--allowedTools`, `--permission-mode acceptEdits`/`dontAsk`/`bypassPermissions`, `--dangerously-skip-permissions` |
| Codex CLI   | `codex exec` "runs in a read-only sandbox" by default. What happens to an approval request in `exec` is not documented                   | `--sandbox workspace-write` / `danger-full-access`, `--dangerously-bypass-approvals-and-sandbox` (`--yolo`)       |
| Gemini CLI  | Policy decision `ask_user`: "In non-interactive mode, this is treated as `deny`"                                                         | `--approval-mode auto_edit` / `yolo`, policy rules; `-y` / `--yolo` is deprecated                                 |
| Aider       | Not documented                                                                                                                           | `--yes-always`                                                                                                    |

No tool skips approvals by default. Where the default is documented, it is "deny that call and keep going", not "wait" and not "abort".

### 2.5 Callbacks and hooks

| Tool        | Mechanism                                                                                                                                    | How status is passed                                                                                                                                                          |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | Hooks in settings files. Events include `Stop`, `Notification`, `PermissionRequest`, `PreToolUse`, `SessionEnd`                              | JSON on stdin (`session_id`, `cwd`, `hook_event_name`, …). Exit 0 applies JSON from stdout; exit 2 blocks; other codes are non-blocking (summary)                             |
| Codex CLI   | `notify` command in user config: "receives a JSON payload from Codex" (summary). A hooks table also exists behind `features.hooks` (summary) | Not documented: when `notify` runs, and whether the payload is an argument or stdin                                                                                           |
| Gemini CLI  | Hooks in `settings.json`. Events include `SessionEnd`, `AfterAgent`, `BeforeTool`, `Notification`                                            | JSON on stdin (`session_id`, `cwd`, `hook_event_name`, `timestamp`, …). Exit 0 parses stdout as JSON; exit 2 blocks; other codes warn. `Notification` is "Observability Only" |
| Aider       | `--notifications-command`: runs when the LLM is done and waiting for input                                                                   | Not documented                                                                                                                                                                |

Where the format is documented, it is one JSON object on the hook's stdin, with exit code 2 reserved for "block".

### 2.6 Exit codes and timeouts

| Tool        | Exit codes                                                                                                               | Timeout for the whole run                                         |
| ----------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| Claude Code | "code 0 on success and a non-zero code when the run fails"; 143 on SIGTERM. The code for `--max-turns` is not documented | None documented (`--max-turns`, `--max-budget-usd` limit the run) |
| Codex CLI   | Not documented                                                                                                           | None documented                                                   |
| Gemini CLI  | `0` success, `1` general error or API failure, `42` input error, `53` turn limit exceeded                                | None documented                                                   |
| Aider       | Not documented                                                                                                           | None documented                                                   |

No tool documents an exit code for "waiting on approval", because none of them waits by default. Only Gemini CLI gives distinct codes at all.

### 2.7 What not to copy

- **Automatic headless on a non-TTY (Gemini CLI).** A silent mode switch makes a script behave differently from the same command typed in a terminal. liblet should require an explicit flag.
- **Short or casual skip flags (`-y`, `--yolo`).** The epic forbids defaulting to approval skipping, and a one-letter flag is easy to add by habit. LibrAgent already has named execution modes; reuse them.
- **Undocumented exit codes (Codex CLI, Aider).** Automation cannot branch on them. liblet's codes must be part of the documented interface.
- **A large flag surface (Claude Code has over 60 options).** Start with the flags below and add on demand.
- **Per-tool-call hooks.** Claude Code and Gemini CLI both have them. The epic lists them as a later item, so phase 1 has one end-or-block callback only.
- **Text-only output (Aider).** A run with no machine-readable result cannot report a session id for resume.

## 3. Proposed liblet interface

liblet is a client of the Session API (`docs/api/http_api.md`). Every behavior below maps to that API or names the gap in section 4.

### 3.1 Invocation

| Command                          | Behavior                                                                             | Convention                                             |
| -------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------ |
| `liblet`                         | Open the TUI on a new session in the current directory                               | All four tools                                         |
| `liblet "prompt"`                | Open the TUI and send `prompt` as the first message                                  | Claude Code, Codex CLI, Gemini CLI                     |
| `liblet -p "prompt"`             | Headless: run one prompt to completion, print the result, exit. Long form `--prompt` | Gemini CLI (`-p` / `--prompt`); Claude Code short form |
| `cat file \| liblet -p "prompt"` | Headless: piped stdin is appended to the prompt as context                           | Claude Code, Codex CLI, Gemini CLI                     |
| `liblet -p -`                    | Headless: read the whole prompt from stdin                                           | Codex CLI                                              |
| `liblet sessions`                | List sessions (id, name, status, workspace path, last message time), then exit       | Gemini CLI `--list-sessions`, as a subcommand          |

Where liblet differs: without `-p`, liblet always wants a terminal. If stdin or stdout is not a TTY and `-p` is absent, liblet prints a usage error and exits with code 2. It does not switch to headless on its own (see 2.7).

### 3.2 Flags

Common to the TUI and headless mode:

| Flag                                    | Meaning                                                                                                                                       |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `-c`, `--continue`                      | Resume the most recent session whose workspace is the current directory                                                                       |
| `-r`, `--resume <session-id>`           | Resume that session from any directory. The session keeps its own workspace path. In a TTY with no id, show a picker                          |
| `-C`, `--cd <dir>`                      | Workspace for a new session. Default: the current directory. Rejected together with `--continue` or `--resume`                                |
| `--assistant <id>`                      | Assistant for a new session                                                                                                                   |
| `--model <name>`, `--provider <name>`   | Model for a new session. Default: the profile's preferred model                                                                               |
| `--name <name>`                         | Display name for a new session                                                                                                                |
| `--isolation host\|docker`              | Workspace isolation for a new session                                                                                                         |
| `--execution-mode normal\|yolo\|unsafe` | Execution mode. Default `normal`. This is the only way to skip approvals, it has no short form, and it uses the existing LibrAgent mode names |
| `--version`, `--help`                   | Standard                                                                                                                                      |

Headless only (`-p`):

| Flag                                      | Meaning                                                                |
| ----------------------------------------- | ---------------------------------------------------------------------- |
| `--output-format text\|json\|stream-json` | Output format. Default `text`                                          |
| `--on-approval deny\|wait\|fail`          | What to do when a tool call needs approval. Default `deny` (see 3.4)   |
| `--timeout <seconds>`                     | Stop waiting after this long and exit with code 124. Default: no limit |
| `--hook <command>`                        | End-or-block callback (see 3.6)                                        |

Profile, data directory, and worker isolation flags belong to instance management (#2701) and are not defined here. liblet honors `LIBRAGENT_DATA_DIR` and the Session API key variables as the runtime does today.

### 3.3 Output

- `text` (default): stdout gets only the final assistant message. Progress, tool activity, and warnings go to stderr. This follows Codex CLI, so `liblet -p "…" > out.md` is clean.
- `json`: stdout gets one JSON object when the run ends.

  ```json
  {
    "type": "result",
    "status": "completed | error | blocked | timeout | interrupted",
    "reason": "approval | question | null",
    "result": "final assistant message (string, may be empty)",
    "error": "message (present when status is error)",
    "sessionId": "a1b2c3d4e5",
    "workspacePath": "/abs/path",
    "executionMode": "normal",
    "deniedApprovals": [
      { "toolCallId": "…", "toolName": "…", "approvalKind": "standard | hard" }
    ],
    "pendingApprovals": [],
    "exitCode": 0
  }
  ```

  `deniedApprovals` and `pendingApprovals` reuse the entry shape of `pendingApprovals` in `GET /api/sessions/:id`. Field names are camelCase to match the Session API; Claude Code and Gemini CLI use snake_case.

- `stream-json`: newline-delimited JSON, one event per line. Event `type` values: `init` (session id, workspace, model), `message` (user and assistant text), `tool_use`, `tool_result`, `approval_request`, `question`, `error` (non-fatal), and a final `result` with the same object as `json`. The first six names follow Gemini CLI; `approval_request` and `question` are added because LibrAgent sessions can block on them.

In every format, `sessionId` is reported so a script can continue with `liblet -p "…" --resume <id>`.

### 3.4 Approvals and user questions in headless mode

The default execution mode is `normal`, so standard and hard approvals are both required, exactly as in the GUI. Nothing is auto-approved unless the caller passes `--execution-mode yolo` or `unsafe`.

When a tool call needs approval (`pendingApprovals` is non-empty):

| `--on-approval`  | Behavior                                                                                                                                                                                                      |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `deny` (default) | liblet answers `deny` through `POST /api/sessions/:id/channel/permission`. The agent continues and can work around the denial. Each denial is listed in `deniedApprovals`. Follows Claude Code and Gemini CLI |
| `wait`           | liblet leaves the approval pending and fires the `blocked` callback once. A person in the GUI, or another client, can answer. The wait is bounded by `--timeout`                                              |
| `fail`           | liblet leaves the approval pending, stops waiting, and exits with code 3. The session stays resumable                                                                                                         |

When the agent asks the user a question (`ui__askUser`) or requests a secret, nobody can answer in headless mode. liblet fires the `blocked` callback with `reason: "question"`, leaves the session as it is, and exits with code 4. `--on-approval wait` does not change this.

### 3.5 Exit codes

| Code  | Meaning                                                                                         | Convention                                   |
| ----- | ----------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `0`   | The run completed. Denied approvals do not change this; they are in `deniedApprovals`           | All tools; Claude Code for denials           |
| `1`   | The run failed: session status `error`, the runtime could not be reached, or an API call failed | Claude Code, Gemini CLI                      |
| `2`   | Usage error: bad flag, bad combination, or no TTY without `-p`. No session is created           | Common shell convention (Gemini CLI uses 42) |
| `3`   | Blocked on a tool approval (`--on-approval fail`)                                               | liblet-specific; no tool documents one       |
| `4`   | Blocked on a user question or secret request                                                    | liblet-specific                              |
| `124` | `--timeout` reached. The session keeps running in the runtime                                   | GNU `timeout`                                |
| `130` | Interrupted by SIGINT. liblet first asks the runtime to terminate the session's workflow        | Shell convention (128 + signal)              |
| `143` | Terminated by SIGTERM, same cleanup                                                             | Claude Code                                  |

Codes 3, 4, and 124 always mean "the session still exists and can be resumed". The `status` field in `json` output carries the same information for callers that do not read exit codes.

### 3.6 End-or-block callback

`--hook <command>` runs `<command>` through the platform shell when the headless run ends or blocks. It is one callback for the whole run, not a per-tool hook.

- **When:** once with `event: "end"` just before liblet exits (any exit code except 2), and once per block with `event: "blocked"` (approval under `--on-approval wait` or `fail`, or a user question).
- **Input:** one JSON object on the command's stdin, the convention shared by Claude Code and Gemini CLI.

  ```json
  {
    "event": "end | blocked",
    "reason": "approval | question | null",
    "status": "completed | error | blocked | timeout | interrupted",
    "exitCode": 0,
    "sessionId": "a1b2c3d4e5",
    "cwd": "/abs/path",
    "result": "final assistant message (end only)",
    "pendingApprovals": []
  }
  ```

- **Output:** observability only in phase 1. The command's stdout is ignored, and it cannot approve, deny, or answer anything. A non-zero exit or a run longer than 60 seconds prints a warning on stderr and does not change liblet's exit code. This is the Gemini CLI `Notification` model. Letting a callback decide approvals is the per-tool hook the epic defers.
- A callback that wants to act on a block can call the Session API itself with `sessionId`, for example the permission endpoint.

### 3.7 TUI requirements that follow from the benchmark

These are inputs to #2702, not a layout.

- A positional prompt starts the TUI with that prompt sent; it never selects headless mode.
- `--continue` and `--resume` behave the same in the TUI and in headless mode.
- An approval request is shown inline with the tool name, reason, and command preview from `pendingApprovals`, and offers allow, deny, and raising the execution mode. Raising the mode is an explicit action, never a default.
- A user question or secret request is answered inline.
- The first Ctrl+C interrupts the running turn; exiting the TUI does not terminate the session unless the user asks.

## 4. Session API gaps this proposal depends on

Checked against `src-tauri/src/server/routes.rs` on `dev/0.9.x` at `dc94e17c3`.

1. **No session list endpoint.** `--continue`, `liblet sessions`, and the resume picker need to list sessions with their workspace path and last message time.
2. **No streaming endpoint.** The API has `GET /api/sessions/:id` and `GET /api/sessions/:id/messages` only. `stream-json` and TUI streaming need either a server-sent event stream or polling; polling is enough for `text` and `json`.
3. **No release-build endpoint for user questions.** Seeing and answering a pending `ui__askUser` prompt exists only in the debug-only E2E control plane (`src-tauri/src/server/e2e_control`). The TUI needs a release endpoint to answer; headless mode needs at least to see that a question is pending to return exit code 4.

These are not part of #2699. They should be assigned to #2700 or to new sub-issues before #2702 and #2703 start.

## 5. Open decisions for a maintainer

Each item has a recommended answer, used in section 3. They are listed here so nothing is decided silently.

1. **Headless flag spelling.** Recommended: `-p` / `--prompt` (Gemini CLI spelling, Claude Code short form). Alternative: a subcommand, `liblet exec "prompt"`, as in Codex CLI.
2. **Default for `--on-approval`.** Recommended: `deny`, the documented default of Claude Code and Gemini CLI. Alternative: `fail`, which never lets a run finish "successfully" after a denial, at the cost of more blocked runs.
3. **No TTY without `-p`.** Recommended: usage error, exit code 2. Alternative: switch to headless automatically, as Gemini CLI does.
4. **Owner of the Session API gaps in section 4.** Recommended: add them to #2700's scope or open sub-issues under #2698.
5. **`--hook` in the TUI.** Recommended: headless only in phase 1. Alternative: also fire `end` when a TUI session exits.
