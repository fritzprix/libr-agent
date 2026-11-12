# liblet CLI interface: benchmark and proposal

Status: proposal for #2699 (part of epic #2698). Nothing here is implemented. Section 5 records the maintainer decisions of 2026-11-12; section 3 follows them and the epic and sub-issues as revised that day. The TUI (#2702) and headless (#2703) tickets should build against section 3.

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
- **Hooks and callbacks.** Claude Code and Gemini CLI both have them. liblet has none: a caller gets the final result, or the full trace, from the process's standard output (decision 5).
- **Text-only output (Aider).** A run with no machine-readable result cannot tell a script why it stopped.

## 3. Proposed liblet interface

### 3.1 Shape

- `liblet` is its own binary. The build produces two artifacts: the GUI app, and `liblet`, a TUI that also supports headless runs. There are no subcommands.
- The GUI and the TUI are equal frontends of one shared local backend and see the same sessions. Whichever starts first starts the backend; a later one connects to it. A session opened in one can be resumed in the other.
- The Session API (`docs/api/http_api.md`) is the contract between that shared backend and its frontends. It is not a way for a thin client to attach to a hidden GUI instance.
- A headless run is self-contained. It hosts its own **ephemeral session** and does not connect to the shared backend. The session is not in the shared session list, cannot be resumed, and is removed when the process exits. Headless runs can proceed in parallel with the GUI, the TUI, and each other.
- A headless run may read its initial settings, including the approval policy, from the profile (see 3.5).
- Several apps may run on one device. Nothing may enforce "one app instance per device"; the only single-owner rule is that a live session has exactly one executor.

### 3.2 Invocation

| Command                          | Terminal                   | Behavior                                                                                       | Convention                                             |
| -------------------------------- | -------------------------- | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `liblet`                         | stdin and stdout are a TTY | Open the TUI on a new session in the current directory                                         | All four tools                                         |
| `liblet "prompt"`                | stdin and stdout are a TTY | Open the TUI and send `prompt` as the first message                                            | Claude Code, Codex CLI, Gemini CLI                     |
| `liblet -p "prompt"`             | Any                        | Headless: run one prompt in an ephemeral session, print the result, exit. Long form `--prompt` | Gemini CLI (`-p` / `--prompt`); Claude Code short form |
| `cat file \| liblet -p "prompt"` | Any                        | Headless: piped stdin is appended to the prompt as context                                     | Claude Code, Codex CLI, Gemini CLI                     |
| `echo "prompt" \| liblet`        | stdin is a pipe            | Headless: the piped text is the prompt                                                         | Codex CLI (`codex exec` with no prompt reads stdin)    |
| `liblet --list-sessions`         | Any                        | Print the shared sessions (id, name, status, workspace path, last message time), then exit     | Gemini CLI                                             |

Mode selection is explicit and has exactly three outcomes:

1. **Headless** when `-p` is given, or when a non-empty prompt arrives on piped stdin and there is no positional prompt.
2. **TUI** when `-p` is absent and both stdin and stdout are a terminal.
3. **Usage error** (exit code 2) otherwise: for example no terminal and no prompt, an empty pipe, or `liblet "prompt"` with stdout redirected.

A positional prompt never selects headless mode. That settles the mixed case: `cat notes.md | liblet "summarize"` has a positional prompt and no terminal, so it is a usage error, and the message tells the user to add `-p`. With `-p`, the piped text is context for the prompt (the `cat file | liblet -p "prompt"` row). Piped text is the prompt itself only when no prompt is given any other way.

liblet does not switch to headless because a terminal is missing (see 2.7). A pipe selects headless only because it carries the prompt.

### 3.3 Flags

Common to the TUI and headless mode (they describe the new session):

| Flag                                    | Meaning                                                                                                                                                               |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-C`, `--cd <dir>`                      | Workspace for the new session. Default: the current directory                                                                                                         |
| `--assistant <id>`                      | Assistant for the new session                                                                                                                                         |
| `--model <name>`, `--provider <name>`   | Model for the new session. Default: the profile's preferred model                                                                                                     |
| `--isolation host\|docker`              | Workspace isolation for the new session                                                                                                                               |
| `--execution-mode normal\|yolo\|unsafe` | Execution mode the session starts with. Default `normal`. This is the only way to skip approvals, it has no short form, and it uses the existing LibrAgent mode names |
| `--version`, `--help`                   | Standard                                                                                                                                                              |

TUI only:

| Flag                          | Meaning                                                                                                   |
| ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| `-c`, `--continue`            | Resume the most recent shared session whose workspace is the current directory                            |
| `-r`, `--resume [session-id]` | Resume that shared session from any directory; it keeps its own workspace path. With no id, show a picker |
| `--name <name>`               | Display name for a new session                                                                            |

`--continue` and `--resume` are rejected with `-C`, and are a usage error in headless mode, because a headless session is ephemeral.

Headless only:

| Flag                                      | Meaning                                                                |
| ----------------------------------------- | ---------------------------------------------------------------------- |
| `--output-format text\|json\|stream-json` | What standard output carries. Default `text` (see 3.4)                 |
| `--timeout <seconds>`                     | Stop the run after this long and exit with code 124. Default: no limit |

There is no `--hook`, no `--on-approval`, and no per-tool allowlist. Profile, data directory, and worker isolation flags belong to instance management (#2701) and are not defined here.

### 3.4 Output

The final result always goes to the process's standard output, the way a parent session receives a sub-session's final result. How much else is printed is fixed when the run starts, by `--output-format`:

- `text` (default): stdout gets only the final assistant message. Progress and warnings go to stderr, so `liblet -p "…" > out.md` is clean. This follows Codex CLI.
- `json`: stdout gets one JSON object when the run ends.

  ```json
  {
    "type": "result",
    "status": "completed | error | blocked | timeout | interrupted",
    "reason": "approval | question | null",
    "result": "final assistant message (string, may be empty)",
    "error": "message (present when status is error)",
    "workspacePath": "/abs/path",
    "executionMode": "normal",
    "pendingApprovals": [
      {
        "toolCallId": "…",
        "toolName": "…",
        "reason": "…",
        "commandPreview": "…",
        "approvalKind": "standard | hard"
      }
    ],
    "exitCode": 0
  }
  ```

  `pendingApprovals` reuses the entry shape of `GET /api/sessions/:id` and is non-empty only when `reason` is `approval`. Field names are camelCase to match the Session API; Claude Code and Gemini CLI use snake_case. There is no session id, because the session does not outlive the run.

- `stream-json`: the **full trace**, as newline-delimited JSON, one event per line. Event `type` values: `init` (workspace, model, execution mode), `message` (user and assistant text), `tool_use`, `tool_result`, `approval_request`, `question`, `error` (non-fatal), and a final `result` with the same object as `json`. `init`, `message`, `tool_use`, `tool_result`, `error`, and `result` follow Gemini CLI; `approval_request` and `question` are added because a LibrAgent turn can stop on them.

### 3.5 Approvals and user questions in headless mode

Approvals follow the configuration the run started with. The starting execution mode is, in order: `--execution-mode` if given; otherwise the approval policy in the profile's initial settings, if the profile sets one; otherwise `normal`. Nothing is auto-approved beyond what that mode already allows, and the mode cannot be raised during the run.

When a tool call needs an approval that the starting mode does not cover, the run **stops**. liblet prints the result with `status: "blocked"`, `reason: "approval"`, and the pending call in `pendingApprovals`, then exits with code 3. It does not deny-and-continue and it does not wait. The caller decides whether to rerun with a higher `--execution-mode`.

When the agent asks the user a question (`ui__askUser`) or requests a secret, nobody can answer. The run stops the same way with `reason: "question"` and exit code 4.

Because the ephemeral session is removed at exit, a stopped run cannot be inspected afterwards. The `pendingApprovals` entry in the result (and the events in a `stream-json` trace) are the only record of what the run was about to do.

Where this differs from the benchmark: Claude Code and Gemini CLI deny the call and let the agent continue. liblet stops, so a run never reports success after working around a refused action.

### 3.6 Exit codes

| Code  | Meaning                                                                                                                                                    | Convention                                   |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `0`   | The run completed                                                                                                                                          | All tools                                    |
| `1`   | The run failed: the session ended in `error` (for example the LLM provider failed), or the run could not start (for example the profile could not be read) | Claude Code, Gemini CLI                      |
| `2`   | Usage error: bad flag, bad combination, or no way to pick a mode (3.2). No session is created                                                              | Common shell convention (Gemini CLI uses 42) |
| `3`   | Stopped: a tool call needed an approval the starting mode does not cover                                                                                   | liblet-specific; no tool documents one       |
| `4`   | Stopped: the agent asked a user question or requested a secret                                                                                             | liblet-specific                              |
| `124` | `--timeout` reached                                                                                                                                        | GNU `timeout`                                |
| `130` | Interrupted by SIGINT                                                                                                                                      | Shell convention (128 + signal)              |
| `143` | Terminated by SIGTERM                                                                                                                                      | Claude Code                                  |

On every exit, liblet terminates the run's workflow and removes the ephemeral session. The `status` field in `json` and `stream-json` output carries the same information for callers that do not read exit codes.

### 3.7 TUI requirements that follow from the benchmark

These are inputs to #2702, not a layout.

- A positional prompt starts the TUI with that prompt sent; it never selects headless mode.
- The TUI lists and resumes the same sessions as the GUI, and a session it creates appears in the GUI.
- An approval request is shown inline with the tool name, reason, and command preview from `pendingApprovals`, and offers allow, deny, and raising the execution mode. Raising the mode is an explicit action, never a default.
- A user question or secret request is answered inline.
- The first Ctrl+C interrupts the running turn; exiting the TUI does not terminate the session unless the user asks.

## 4. What is still missing

Checked against `src-tauri/src/server/routes.rs` on `dev/0.9.x` at `dc94e17c3`.

### 4.1 Shared-backend contract (TUI)

The TUI cannot be a full frontend without these. All three are in #2700's work list as revised on 2026-11-12.

1. **No session list.** `--continue`, `--list-sessions`, and the resume picker need the shared sessions with their workspace path and last message time.
2. **No streaming.** The API has `GET /api/sessions/:id` and `GET /api/sessions/:id/messages` only. The TUI needs an event stream for model output.
3. **No release-build route for user questions.** Seeing and answering a pending `ui__askUser` prompt exists only in the debug-only E2E control plane (`src-tauri/src/server/e2e_control`).

### 4.2 Headless requirement (#2703)

There is no ephemeral session today: a session is persisted and visible to every frontend. #2703 has to provide a session that a headless process hosts itself, that never enters the shared list, and that is removed when the run ends. This is not a gap in the shared-backend contract, because a headless run does not use that backend. The `stream-json` trace comes from the headless process's own run, not from a backend stream.

## 5. Maintainer decisions (2026-11-12)

The first draft listed five open decisions. They are settled as follows.

1. **Headless flag.** `-p` / `--prompt`, the familiar spelling. No subcommand is needed.
2. **Approvals in headless mode.** Approvals follow the configuration the run started with. If a further approval is needed, the run stops (fail). Approvals are never skipped by default.
3. **Mode selection.** No automatic headless. The TUI runs only when there is a terminal. Headless runs only with `-p` or when the prompt is piped in. With neither, liblet exits with a usage error.
4. **Architecture.** liblet is an independent binary. The build produces one GUI and one TUI that supports headless. The TUI and the GUI share the backend and the sessions; headless uses a separate, ephemeral session. A single-instance lock that would block running several apps on one device is removed; only the executor of a live session is single. The Session API is needed only as the contract between the shared backend and its frontends, not as a way for an extra client to attach to the GUI.
5. **Callbacks and output.** `--hook` is not supported. The final result goes to the process's standard output, as when a sub-session returns its final result. A full trace must be available according to the initial configuration.

### 5.1 How section 3 reads those decisions

These are this document's interpretations. Correct them here if they are wrong.

- **"No subcommand" is applied to the whole CLI.** Session listing is the flag `--list-sessions`, not `liblet sessions`.
- **"Ephemeral" is read as: not listed, not resumable, removed at exit.** So headless output carries no session id, and `--continue` / `--resume` are TUI-only.
- **"Initial configuration" is read as what is fixed when the run starts:** the flags, then the profile's initial settings. `--execution-mode` (or the profile's approval policy) fixes approvals for the run, and `--output-format stream-json` is the full trace.
- **A user question stops a headless run the same way an approval does** (exit code 4). Decision 2 names approvals only.
- **`--timeout` is kept.** It was not part of the five decisions.

### 5.2 State of the epic and sub-issues

The maintainer revised #2698, #2700, #2701, #2702, and #2703 on 2026-11-12 to match these decisions. Nothing in them is known to be out of date:

- **#2698** describes equal frontends on a shared backend, ephemeral headless, and no `--hook`.
- **#2700** forbids a single-instance or data-dir lock and adds the session list, streaming, and user-question routes of section 4.1.
- **#2701** is the shared backend for the GUI and the TUI, with headless out of scope.
- **#2703** is the ephemeral headless path: `-p` or a piped prompt, result on stdout, no callback, a distinct exit status when a human is needed.
- **#2702** says reserved subcommands take precedence "if the interface proposal keeps that shape". This proposal has no subcommands (5.1), so that clause does not apply.
