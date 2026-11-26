/**
 * Per-server store for in-flight OAuth authorization flows (#3040 G2.2).
 *
 * The flow promise lives here, outside any card, so a remounted ServerCard
 * can join the flow it did not start and learns when it ends — instead of
 * showing Cancel forever with no way back to Authorize.
 */

export type AuthFlowOutcome =
  | { kind: 'success' }
  | { kind: 'failure'; message: string };

export interface AuthFlowRecord {
  /** Resolves when the flow ends, with success or the failure message. */
  promise: Promise<AuthFlowOutcome>;
  /** null while the flow is running; set once it settles. */
  settled: AuthFlowOutcome | null;
}

const flows = new Map<string, AuthFlowRecord>();
const listeners = new Map<string, Set<() => void>>();

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

/** Test hook: forget every flow. Production code must not call this. */
export function __resetAuthFlowsForTest(): void {
  flows.clear();
  listeners.clear();
}

function notify(serverId: string): void {
  const set = listeners.get(serverId);
  if (!set) return;
  for (const listener of [...set]) listener();
}
