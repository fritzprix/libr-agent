/**
 * Timeouts and retries for self-hosted / local OpenAI-compatible endpoints.
 *
 * Large context prefills (e.g. ~128k at session start) commonly take 5–6+
 * minutes on local llama.cpp / Ollama hosts. Cloud-oriented short timeouts and
 * retries create duplicate in-flight completions and GPU contention.
 */

/** Allow slow local prefills with headroom beyond typical 5–6 minute runs. */
export const SELF_HOSTED_LLM_TIMEOUT_MS = 900_000; // 15 minutes

/** Never auto-retry slow local prefills — retries duplicate GPU work. */
export const SELF_HOSTED_LLM_MAX_RETRIES = 0;

/**
 * True when `baseUrl` points at a loopback, private-network, or .local host.
 * Empty / invalid URLs are treated as cloud (not self-hosted).
 * Scheme-less values like `127.0.0.1:11434` are normalized with `http://`.
 */
export function isSelfHostedLlmBaseUrl(
  baseUrl: string | undefined | null,
): boolean {
  if (!baseUrl || !baseUrl.trim()) {
    return false;
  }

  const trimmed = baseUrl.trim();
  const normalized = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(trimmed)
    ? trimmed
    : `http://${trimmed}`;

  let url: URL;
  try {
    url = new URL(normalized);
  } catch {
    return false;
  }

  const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, '');

  if (
    host === 'localhost' ||
    host === '127.0.0.1' ||
    host === '::1' ||
    host === '0.0.0.0' ||
    host === 'host.docker.internal'
  ) {
    return true;
  }

  if (host.endsWith('.local')) {
    return true;
  }

  // Private IPv4 ranges (RFC 1918)
  if (/^10\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(host)) {
    return true;
  }
  if (/^192\.168\.\d{1,3}\.\d{1,3}$/.test(host)) {
    return true;
  }
  if (/^172\.(1[6-9]|2\d|3[0-1])\.\d{1,3}\.\d{1,3}$/.test(host)) {
    return true;
  }

  return false;
}

/**
 * Defaults for self-hosted OpenAI-compatible / Ollama clients.
 * - `config.timeout` wins when set; otherwise 15 minutes.
 * - `config.maxRetries` is ignored: retries are always forced to 0 so settings'
 *   cloud retry policy cannot duplicate local prefills.
 */
export function resolveSelfHostedLlmClientOptions(
  baseUrl: string | undefined | null,
  config?: { timeout?: number; maxRetries?: number },
): { timeout: number; maxRetries: number } | undefined {
  if (!isSelfHostedLlmBaseUrl(baseUrl)) {
    return undefined;
  }

  return {
    timeout: config?.timeout ?? SELF_HOSTED_LLM_TIMEOUT_MS,
    maxRetries: SELF_HOSTED_LLM_MAX_RETRIES,
  };
}
