/**
 * LLM transport fetch that bypasses webview CORS in Tauri desktop.
 *
 * OpenAI-compatible hosts (e.g. NVIDIA Integrate) often omit
 * Access-Control-Allow-Origin. Browser fetch then fails while native HTTP
 * succeeds. Prefer @tauri-apps/plugin-http in the Tauri runtime.
 *
 * Some hosts (corporate PAC / WinINET vs WinHTTP) are reachable from the
 * webview but hang or time out via reqwest. On transport-level failures,
 * fall back to globalThis.fetch and remember the working transport per origin.
 *
 * Native attempts are bounded by a short probe timeout so fallback can finish
 * inside callers' outer listModels budgets (typically withTimeout 20s).
 */
import { fetch as tauriPluginFetch } from '@tauri-apps/plugin-http';
import { getLogger } from '@/lib/logger';

const logger = getLogger('DesktopFetch');

export type LlmFetch = typeof globalThis.fetch;

type LlmTransport = 'native' | 'browser';

/**
 * Max wait for plugin-http before falling back to webview fetch.
 * Must stay well under listModels `withTimeout` (20s) so auto-load and
 * manual refresh behave consistently.
 */
export const DEFAULT_NATIVE_TRANSPORT_PROBE_MS = 8_000;

/** Successful transport preference keyed by request origin. */
const transportByOrigin = new Map<string, LlmTransport>();

/**
 * Clears sticky transport preferences (tests / diagnostics).
 */
export function clearLlmFetchTransportPreferences(): void {
  transportByOrigin.clear();
}

/**
 * Returns the sticky transport for an origin, if learned.
 */
export function getLlmFetchTransportPreference(
  origin: string,
): LlmTransport | undefined {
  return transportByOrigin.get(origin);
}

/**
 * True when running inside a real Tauri webview (not Vitest stubs).
 * Test setup defines `__TAURI_INTERNALS__`, so MODE=test must stay on browser fetch.
 */
export function isTauriLlmFetchRuntime(
  env: ImportMetaEnv = import.meta.env,
): boolean {
  // Vitest stubs `__TAURI_INTERNALS__`; never route LLM traffic through plugin-http in tests.
  if (env.MODE === 'test' || 'VITEST' in env) {
    return false;
  }
  return (
    typeof window !== 'undefined' &&
    Object.prototype.hasOwnProperty.call(window, '__TAURI_INTERNALS__')
  );
}

/**
 * Resolve the origin used for sticky transport selection.
 */
export function resolveLlmFetchOrigin(input: RequestInfo | URL): string {
  if (typeof input === 'string') {
    return new URL(input).origin;
  }
  if (input instanceof URL) {
    return input.origin;
  }
  return new URL(input.url).origin;
}

/**
 * True for transport/connectivity failures where webview fetch may still work.
 * HTTP status errors are returned as Response and must not trigger fallback.
 */
export function isLlmTransportNetworkError(error: unknown): boolean {
  if (error instanceof DOMException && error.name === 'AbortError') {
    return false;
  }
  if (error instanceof Error && error.name === 'AbortError') {
    return false;
  }

  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : '';

  if (!message) {
    return false;
  }

  const normalized = message.toLowerCase();
  return (
    normalized.includes('timed out') ||
    normalized.includes('timeout') ||
    normalized.includes('operation timed out') ||
    normalized.includes('connection error') ||
    normalized.includes('error sending request') ||
    normalized.includes('failed to fetch') ||
    normalized.includes('networkerror') ||
    normalized.includes('network request failed') ||
    normalized.includes('connection reset') ||
    normalized.includes('connection refused') ||
    normalized.includes('econnrefused') ||
    normalized.includes('econnreset') ||
    normalized.includes('enotfound') ||
    normalized.includes('dns')
  );
}

function splitRequestForRetry(input: RequestInfo | URL): {
  nativeInput: RequestInfo | URL;
  browserInput: RequestInfo | URL;
} {
  if (typeof Request !== 'undefined' && input instanceof Request) {
    // Clone so a failed native attempt does not consume the body for fallback.
    return {
      nativeInput: input.clone(),
      browserInput: input,
    };
  }
  return { nativeInput: input, browserInput: input };
}

/**
 * Race a promise against a timeout without aborting the underlying work.
 * Used so a hanging plugin-http call cannot block browser fallback forever.
 */
export function raceWithTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  timeoutMessage = 'Operation timed out',
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(timeoutMessage));
    }, timeoutMs);

    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

async function fetchWithNativeFallback(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  nativeProbeTimeoutMs: number,
): Promise<Response> {
  const origin = resolveLlmFetchOrigin(input);
  const preferred = transportByOrigin.get(origin);

  if (preferred === 'browser') {
    return globalThis.fetch(input, init);
  }

  const { nativeInput, browserInput } = splitRequestForRetry(input);

  try {
    const response = await raceWithTimeout(
      tauriPluginFetch(nativeInput, init),
      nativeProbeTimeoutMs,
    );
    transportByOrigin.set(origin, 'native');
    return response;
  } catch (nativeError) {
    if (!isLlmTransportNetworkError(nativeError)) {
      throw nativeError;
    }

    logger.warn(
      `plugin-http failed for ${origin}; falling back to webview fetch`,
      nativeError,
    );

    try {
      const response = await globalThis.fetch(browserInput, init);
      transportByOrigin.set(origin, 'browser');
      return response;
    } catch (browserError) {
      // Attach without Error.cause typing (needs ES2022 lib); keep chain for logs.
      if (browserError instanceof Error && nativeError instanceof Error) {
        Object.defineProperty(browserError, 'cause', {
          value: nativeError,
          configurable: true,
          writable: true,
        });
      }
      throw browserError;
    }
  }
}

/**
 * Returns fetch for LLM SDKs and raw provider calls.
 * Tauri → plugin-http first (bounded probe), webview fetch on transport failure.
 * Non-Tauri → globalThis.fetch.
 */
export function createLlmFetch(
  options: {
    isTauri?: boolean;
    /** Override native probe timeout (ms). Defaults to DEFAULT_NATIVE_TRANSPORT_PROBE_MS. */
    nativeProbeTimeoutMs?: number;
  } = {},
): LlmFetch {
  const useTauri =
    options.isTauri !== undefined ? options.isTauri : isTauriLlmFetchRuntime();

  if (!useTauri) {
    return globalThis.fetch.bind(globalThis);
  }

  const nativeProbeTimeoutMs =
    options.nativeProbeTimeoutMs ?? DEFAULT_NATIVE_TRANSPORT_PROBE_MS;

  return (input, init) =>
    fetchWithNativeFallback(input, init, nativeProbeTimeoutMs);
}
