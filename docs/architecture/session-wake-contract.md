# Session Wake Contract

Source: the contract in #2343 and the owner decisions in
https://github.com/fritzprix/libr-agent/pull/2346#issuecomment-6441997380. This document adds no
decision of its own.

## What call-me-back is

call-me-back lets an **external process wake a session asynchronously**: the agent starts
something, tells the user, ends its turn, and the process later reports back with an HTTP
callback that arrives as a message in the same session. It is not `workspace__waitForProcess`,
which blocks the turn until a process ends. Use `waitForProcess` when the agent should wait in
the turn; use a wake callback when the event comes from outside and the agent should be idle
until it fires.

## Where the callback lands

`runShell` / `spawnProcess` export `$LIBRAGENT_WAKE_URL`, a URL carrying this session's token. It
has the same form in every mode: `http://127.0.0.1:<port>/api/sessions/<id>/wake/<token>`, where
`<port>` is the host's active Session API port (`active_http_port()`).

| Session mode | How the callback reaches the Session API |
| --- | --- |
| Host | Host loopback, the Session API itself |
| Managed Docker | The container's `127.0.0.1:<port>` is served by a relay inside the container, which forwards to the session's unix socket, which appears at `/run/libragent/api.sock`; the host serves that socket with the same router as the main port |

The host opens no extra TCP port for containers. Each managed session has its own socket directory
(`~/.libragent/sock/<session_id>/`, or under the data directory when one is set). Inside the container, the
socket appears at `/run/libragent/api.sock`. How it gets there depends on the platform, decided in one
place (`session_socket::socket_mount_kind`):

| Platform | Mount | After an app restart |
| --- | --- | --- |
| Linux, Windows | the session's socket directory at `/run/libragent/` | the container is kept: the re-created `api.sock` is visible in it |
| macOS | only the socket file at `/run/libragent/api.sock` (Docker Desktop cannot connect to a socket reached through a mounted directory) | the container is **re-created**: the file mount pins the old socket, and the app binds a new one on every start |

The relay is a static musl binary (x86_64, aarch64) embedded in the app; the host starts it with
`docker exec`, waits for it to print `ready`, and registers it only then. The relay prints `ready` once it
is listening **and** one probe connection to `/run/libragent/api.sock` succeeded, so `ready` means a request
to the port reaches the Session API. The session is ready only after that: provisioning waits for it (step
"Starting session relay") after releasing the session's provisioning lock, so the first command already sees
`$LIBRAGENT_WAKE_URL` and nothing else waits behind a slow relay. It needs nothing from the image.

## Permissions of a socket-origin request

A request that arrives on a session's socket has that session as its **origin**, fixed when the
socket is served. Requests on the main TCP port have no origin and behave as before.

- Session routes are allowed for the origin session and its direct children; any other session
  gets `403`.
- Deleting or terminating is allowed for a direct child only, never the origin itself.
- `POST /api/sessions/:id/wake/:token` is authorized by the token alone, not by ownership.
- `channel/permission` approval, `/mcp/*`, automatic channel selection and settings writes are
  always `403`; approvals happen only on the main port, by a person.
- Non-canonical paths (empty segments, `.` / `..`, `%`, `\`, `;`) are denied before any rule is
  looked up.
- Creating a session through the socket is limited to a default managed Docker session
  (`workspaceIsolation: docker`, only `dockerConfig.image`, normal execution mode, parent fixed to
  the origin, no org fields). The existing concurrency gate applies, and a requested `maxDepth` /
  `maxFanout` above the inherited value is `403`; the baseline comes from
  `advancedSettings.defaultSessionMaxDepth` / `defaultSessionMaxFanout`, where `0` means
  unlimited.

## Security conditions

- **Per-session token.** The token is minted per session, checked in constant time before the
  payload is parsed, and revoked when the session ends. A token from another session is rejected.
- **Socket access.** The socket directory is `0700` and the socket `0600`. A container sees only
  its own session's socket.
- **No token in the host process list.** For Docker, the runtime passes
  `docker exec -e LIBRAGENT_WAKE_URL` with no value and sets the value in the docker CLI's own
  environment, so the token never appears in `ps` output.
- **No token in tool output.** Tool results pass through a redaction step that removes any
  `/wake/<token>` path and the session's token.

## When it does not work

- **No relay, no URL.** `$LIBRAGENT_WAKE_URL` is set only while the relay is serving the port. If
  the container's port is already in use (exit code 3), the relay cannot reach the socket (exit code 5),
  or the build embeds no relay, no URL is given and a warning is logged.
- **A failure is remembered.** A start that failed is not tried again while the container, the port and the
  socket binding stay the same: no `docker cp` / `docker exec` on every `runShell`. Removing or re-creating
  the container, a new port, or a new socket binding allows one new attempt.
- **Relay died.** Every `runShell` of a Docker session re-checks the relay in the background (one check at
  a time per session; the command does not wait). A relay that exited (including exit code 4 when it can no
  longer accept connections) is noticed there and started again, so the next `runShell` sees the corrected
  state. A relay that runs for another port or socket than the current ones (the host fell back to another
  port after a restart) is stopped and replaced; only a process whose command line is the relay's path with
  the current port and socket counts as the running relay.
- **Starts and removal.** Each start passes the relay an instance id. A start that finishes after the
  container was forgotten registers nothing and stops only its own relay, never one a newer start launched.
- **Host restart & container preservation.** With the directory mount (Linux, Windows), the host recreates
  `api.sock` in the existing directory, which is immediately visible inside the container without container
  recreation. With the file mount (macOS) the container is re-created on the first use after a restart.
- **Stale or missing mount.** The `com.libragent.socket_mount` label records the mount kind and path (and, for
  a file mount, the socket binding). A container whose label is missing or differs is recreated on next use
  with a warning. State outside the mounts is lost.
- **Windows.** No socket and no wake URL for Docker sessions for now.

## How the bench checks this

Harbor ends a trial when the agent goes idle, so the wake callback itself is never judged.
Layman T04 grades only that the agent registered a subscription whose command calls
`$LIBRAGENT_WAKE_URL` with `curl` or `wget`, then went idle. T04 holds in `host` and `managed`
mode; where the variable is unset, the `call-me-back` skill rule "no variable, don't use it"
applies.
