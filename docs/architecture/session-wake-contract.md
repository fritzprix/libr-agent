# Session Wake Contract

## What call-me-back is

call-me-back lets an **external process wake a session asynchronously**: the agent starts
something, tells the user, ends its turn, and the process later reports back with an HTTP
callback that arrives as a message in the same session. It is not `workspace__waitForProcess`,
which blocks the turn until a process ends. Use `waitForProcess` when the agent should wait in
the turn; use a wake callback when the event comes from outside and the agent should be idle
until it fires.

## Where the callback lands

`runShell` / `spawnProcess` export `$LIBRAGENT_WAKE_URL`, a URL carrying this session's token.
The URL depends on the session's workspace mode:

| Session mode | `$LIBRAGENT_WAKE_URL` points to | Path to the Session API |
| --- | --- | --- |
| Host | `http://127.0.0.1:<port>/api/sessions/<id>/wake/<token>` | Host loopback, the Session API itself |
| Managed Docker | `http://host.docker.internal:<wake-port>/api/sessions/<id>/wake/<token>` | The wake-only listener on the host, reached through `host.docker.internal` |

The Session API listens on host loopback only, which a container cannot reach. For Docker
sessions the app starts a second, wake-only listener (`src-tauri/src/server/wake_listener.rs`)
on an ephemeral port. New managed containers are created with
`--add-host host.docker.internal:host-gateway` so the name resolves on Linux; Docker Desktop
(macOS, Windows) resolves it already. A Docker session without a wake listener (it failed to
start) gets no `$LIBRAGENT_WAKE_URL`.

Managed containers created before this listener existed lack the `--add-host` entry. On Linux,
`host.docker.internal` does not resolve in them, so recreate the container to get a working wake
path.

## Security conditions

- **Wake only.** The wake-only listener serves `POST /api/sessions/:id/wake/:token` and nothing
  else. Every other path and method is rejected before any handler runs. The rest of the Session
  API is not exposed to containers.
- **Per-session token.** The token is 256 bits, minted per session, checked in constant time
  before the payload is parsed, and revoked when the session ends. A token from another session
  gets `401`, and so does an unknown session, so callers learn nothing.
- **Source filter.** On Linux the listener binds all interfaces (a container reaches the host
  through the bridge gateway). It answers only loopback, private-network (RFC 1918), and
  link-local sources; other sources get `404`. On macOS and Windows it binds loopback only.
- **No token in the host process list.** For Docker, the runtime passes
  `docker exec -e LIBRAGENT_WAKE_URL` with no value and sets the value in the docker CLI's own
  environment, so the token never appears in `ps` output.
- **No token in tool output.** Tool results pass through a redaction step that removes any
  `/wake/<token>` path and the session's token, whatever the host name in the URL.

## How the bench checks this

Harbor ends a trial when the agent goes idle, so the wake callback itself is never judged.
Layman T04 (`t04-event-subscribe`) grades only that the agent registered a subscription whose
command calls `$LIBRAGENT_WAKE_URL` with `curl` or `wget`, then went idle. T04 holds in `host`
mode. In `managed` mode it needs the wake path above; without it the variable is missing and the
`call-me-back` skill rule "no variable, don't use it" applies.

## Troubleshooting

- **Linux firewall.** A host firewall (for example `ufw`) that drops traffic from the Docker
  bridge to arbitrary host ports makes the callback fail silently. Allow the bridge subnet to the
  wake port, or check with `curl -v "$LIBRAGENT_WAKE_URL"` from inside the container.
- **Custom networks.** Containers on a user-defined network outside the private ranges above are
  rejected by the source filter.
