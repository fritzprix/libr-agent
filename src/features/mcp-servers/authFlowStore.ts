/**
 * Per-server store for in-flight OAuth authorization flows (#3040 G2.2 / G3.1).
 *
 * The flow promise lives here, outside any card, so a remounted ServerCard
 * can join the flow it did not start and learns when it ends — instead of
 * showing Cancel forever with no way back to Authorize.
 *
 * Flows that survive an app reload have no store promise; the backend emits
 * `mcp-oauth-flow-ended` (and keeps a queryable last outcome) so every card
 * still learns the terminal ending without polling (#3040 G3.1).
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type AuthFlowOutcome =
  | { kind: 'success' }
  | { kind: 'failure'; message: string };

export interface AuthFlowRecord {
  /** Resolves when the flow ends, with success or the failure message. */
  promise: Promise<AuthFlowOutcome>;
  /** null while the flow is running; set once it settles. */
  settled: AuthFlowOutcome | null;
}

/** Matches `OAuthFlowEndedPayload` from the Rust backend (#3040 G3.1). */
export interface OAuthFlowEndedPayload {
  serverId: string;
  success: boolean;
  message?: string;
}

const flows = new Map<string, AuthFlowRecord>();
const listeners = new Map<string, Set<() => void>>();

let unlistenFlowEnded: UnlistenFn | null = null;
let listenStarted = false;

export function getAuthFlow(serverId: string): AuthFlowRecord | null {
  return flows.get(serverId) ?? null;
}

/**
 * Starts the authorization flow for `serverId`, or joins the one already
 * running (single-flight matches the backend's shared callback port).
 * `started` is true only for the initiator, so exactly one card reports the
 * toasts and probe.
 */
export function beginAuthFlow(
  serverId: string,
  run: () => Promise<AuthFlowOutcome>,
): { record: AuthFlowRecord; started: boolean } {
  const existing = flows.get(serverId);
  if (existing && existing.settled === null) {
    return { record: existing, started: false };
  }

  // Build the record around the real promise — no placeholder that gets
  // overwritten (#3040 S3.1). The `then` callback runs asynchronously, so
  // `record` is initialized before it reads it.
  const promise = run().then((outcome) => {
    record.settled = outcome;
    notify(serverId);
    flows.delete(serverId);
    return outcome;
  });
  const record: AuthFlowRecord = { promise, settled: null };
  flows.set(serverId, record);
  return { record, started: true };
}

/**
 * Apply a backend terminal outcome for cards that have no local promise
 * (reload / remount onto a backend-held flow, #3040 G3.1). If this session
 * still owns an unsettled store record, leave it alone — that promise is the
 * source of truth for the initiating card.
 */
export function applyBackendFlowOutcome(payload: OAuthFlowEndedPayload): void {
  const existing = flows.get(payload.serverId);
  if (existing && existing.settled === null) {
    return;
  }

  const outcome: AuthFlowOutcome = payload.success
    ? { kind: 'success' }
    : {
        kind: 'failure',
        message:
          payload.message ??
          'the authorization ended without storing a token',
      };

  const promise = Promise.resolve(outcome);
  const record: AuthFlowRecord = { promise, settled: outcome };
  flows.set(payload.serverId, record);
  notify(payload.serverId);
  // Match beginAuthFlow: drop after subscribers have read `settled`.
  flows.delete(payload.serverId);
}

export function subscribeToAuthFlow(
  serverId: string,
  listener: () => void,
): () => void {
  let set = listeners.get(serverId);
  if (!set) {
    set = new Set();
    listeners.set(serverId, set);
  }
  set.add(listener);
  const target = set;
  return () => {
    target.delete(listener);
    if (target.size === 0) listeners.delete(serverId);
  };
}

/**
 * Start listening for backend flow endings once (idempotent). Cards call this
 * on mount so a reload path receives `mcp-oauth-flow-ended` (#3040 G3.1).
 */
export function ensureOAuthFlowEndedListener(): void {
  if (listenStarted) return;
  listenStarted = true;
  try {
    void Promise.resolve(
      listen<OAuthFlowEndedPayload>('mcp-oauth-flow-ended', (event) => {
        applyBackendFlowOutcome(event.payload);
      }),
    )
      .then((unlisten) => {
        unlistenFlowEnded = unlisten;
      })
      .catch(() => {
        // Non-Tauri / test environments: cards still use applyBackendFlowOutcome.
        listenStarted = false;
      });
  } catch {
    listenStarted = false;
  }
}

/** Test hook: forget every flow. Production code must not call this. */
export function __resetAuthFlowsForTest(): void {
  flows.clear();
  listeners.clear();
  if (unlistenFlowEnded) {
    unlistenFlowEnded();
    unlistenFlowEnded = null;
  }
  listenStarted = false;
}

function notify(serverId: string): void {
  const set = listeners.get(serverId);
  if (!set) return;
  for (const listener of [...set]) listener();
}
