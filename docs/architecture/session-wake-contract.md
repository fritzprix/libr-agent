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
| Managed Docker | The container's `127.0.0.1:<port>` is served by a relay inside the container, which forwards to the session's unix socket mounted at `/run/libragent/api.sock`; the host serves that socket with the same router as the main port |

The host opens no extra TCP port for containers. Each managed session has its own socket
(`~/.libragent/sock/<session_id>.sock`, or under the data directory when one is set), mounted
into its container as a single file. The relay is a static musl binary (x86_64, aarch64) embedded
in the app and started with `docker exec -d`; it needs nothing from the image.

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
  the container's port is already in use, the relay is not started, no URL is given and a warning
  is logged. A local build without the embedded relay behaves the same way.
- **Relay died.** The next `runShell` checks the relay and starts it again.
- **Stale or missing mount.** A container without the socket mount, or whose mount is stale after
  a host restart, is recreated on next use with a warning. State outside the mounts is lost.
- **Windows.** No socket and no wake URL for Docker sessions for now.

## How the bench checks this

Harbor ends a trial when the agent goes idle, so the wake callback itself is never judged.
Layman T04 grades only that the agent registered a subscription whose command calls
`$LIBRAGENT_WAKE_URL` with `curl` or `wget`, then went idle. T04 holds in `host` and `managed`
mode; where the variable is unset, the `call-me-back` skill rule "no variable, don't use it"
applies.
