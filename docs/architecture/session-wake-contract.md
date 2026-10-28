# Session Wake Callback Contract

Single source of truth for how an external process wakes a LibrAgent session
(call-me-back). Implementation: `src-tauri/src/server/wake.rs` (tokens),
`src-tauri/src/server/handlers/wake.rs` (route). Other docs link here and do not
restate the contract.

## Definition

call-me-back lets a process outside the session report an event asynchronously and
wake the session with it. The session ends its turn after registering the callback
and is resumed by the incoming message.

It is not `waitForProcess`. That tool blocks the turn until a process exits and
works without any network path. call-me-back frees the turn and needs a reachable
wake URL.

## Wake route

```
POST /api/sessions/:id/wake/:token
```

- `:token` is the session's wake token (see [Security](#security)). It is the only
  credential the route checks.
- Body is optional JSON; every field is optional, so a bodyless `POST` wakes the
  session: `status` (`success` / `failure`, else derived from `exitCode`),
  `exitCode`, `summary`, `logPath`.
- The session receives a `[wake callback]` message. A busy session queues it.
- Responses: `200` accepted, `400` invalid payload, `401` invalid or expired token,
  `410` session has ended.

## `$LIBRAGENT_WAKE_URL`

A full, tokenized wake URL for **this** session only:
`http://<host>:<port>/api/sessions/<id>/wake/<token>`.

| Session mode | `<host>:<port>` | Reached through |
| --- | --- | --- |
| host | `127.0.0.1:<Session API port>` | host loopback, main Session API server |
| managed Docker | `host.docker.internal:<wake listener port>` | wake listener (below) |

- Injected only into the environments of `runShell` / `spawnProcess` started by the
  session. It is not in the model's context; the model refers to it by name.
- Managed Docker containers are created with
  `--add-host=host.docker.internal:host-gateway`. Linux needs it; Docker Desktop
  (macOS, Windows) provides the name already, so no per-platform branch exists.
  Containers created without the mapping must be recreated.
- When no wake URL can be built for a session (for example the wake listener is not
  running), the variable is not injected and the failure is reported explicitly,
  never replaced by an unreachable URL.

## Exposure

- The main Session API server binds `127.0.0.1` only.
- The app runs a second, **wake-only** HTTP listener. Only the wake route is
  registered on it; every other path returns `404`. Containers reach this listener
  and nothing else.
- The URL carries the port that was actually bound, not a configured one.
- Non-wake routes are therefore unreachable from non-loopback callers by
  construction, not by a filter on a shared server.

## Security

Authorization:

- The wake route is authorized by the session's token from the path alone. The
  Session API key, if configured, is neither required nor accepted in its place.
- Every other Session API route requires the Bearer key when one is configured;
  without a key it is loopback-only.
- Containers get the wake URL and never the Session API key.

Token:

- Generated from a CSPRNG, valid for one session.
- Kept in memory only. It is invalidated by app restart and by session terminate or
  delete; late callbacks get `401`.
- Compared in constant time.
- Never appears in host process lists, logs or model output. Tool results are
  redacted before they reach the model, the UI or stored history.

The wider Session API permission model (app approval, per-route permissions) is
tracked in #2348 and is out of scope here. The wake route's exemption from the key
guard must survive that work.

## Verification (bench)

Harbor `layman` task T04 (`benchmarks/harbor/libragent-layman-intent/t04-event-subscribe`)
checks the contract in both `host` and `managed` workspace modes.

- The verifier checks the mock monitor's recorded subscription: its hook must be a
  `curl` / `wget` request to the wake URL (via `$LIBRAGENT_WAKE_URL` or its literal
  value). The verifier container has no LibrAgent, so it cannot fire the hook.
- A recorded subscription alone is not proof of the transport. For `managed` mode,
  validation also fires the registered hook from inside the container and confirms
  the session wakes (tracked in #2360).
- Negative checks: a wake with another session's token is rejected, and a request
  from the container to any non-wake Session API route is refused.

## Open

- Bind address of the wake listener (all interfaces vs. the Docker bridge gateway).
  Decided in #2358 after measuring on each platform, then recorded here.
