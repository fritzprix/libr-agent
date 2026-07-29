/**
 * Structured logging helpers when dynamic model listing fails and the UI
 * falls back to static llm-config models.
 */

import { toast } from 'sonner';

export interface ListModelsFailureContext {
  provider: string;
  baseUrl?: string | null;
  reason: 'api_error' | 'invalid_response';
  error?: unknown;
  hasCachedModels?: boolean;
}

export interface ListModelsFailurePayload {
  reason: ListModelsFailureContext['reason'];
  provider: string;
  baseUrl: string | null;
  message: string;
  usedStaticFallback: true;
  hasCachedModels?: boolean;
}

type ListModelsFallbackListener = (payload: ListModelsFailurePayload) => void;

const fallbackListeners = new Set<ListModelsFallbackListener>();

let lastToastKey = '';
let lastToastAt = 0;
const TOAST_DEDUP_MS = 5_000;

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/**
 * Build a stable payload for warn/error logs (and UI toast messages).
 */
export function describeListModelsFailure(
  context: ListModelsFailureContext,
): ListModelsFailurePayload {
  return {
    reason: context.reason,
    provider: context.provider,
    baseUrl: context.baseUrl ?? null,
    message: errorMessage(context.error),
    usedStaticFallback: true,
    hasCachedModels: context.hasCachedModels,
  };
}

/**
 * User-facing short message when API model listing fails.
 */
export function listModelsFailureToastMessage(
  provider: string,
  error: unknown,
): string {
  const detail = errorMessage(error);
  return detail
    ? `Failed to fetch models for ${provider}: ${detail}`
    : `Failed to fetch models for ${provider}`;
}

/**
 * Subscribe to static-fallback events from listModels (for settings / agent UI).
 */
export function subscribeListModelsFallback(
  listener: ListModelsFallbackListener,
): () => void {
  fallbackListeners.add(listener);
  return () => {
    fallbackListeners.delete(listener);
  };
}

function notifyFallbackListeners(payload: ListModelsFailurePayload): void {
  for (const listener of fallbackListeners) {
    listener(payload);
  }
}

function toastListModelsFallback(payload: ListModelsFailurePayload): void {
  // If persistent cached models are available for this provider, do not alarm the user with error toasts
  if (payload.hasCachedModels) {
    return;
  }
  const key = `${payload.provider}:${payload.message}`;
  const now = Date.now();
  if (key === lastToastKey && now - lastToastAt < TOAST_DEDUP_MS) {
    return;
  }
  lastToastKey = key;
  lastToastAt = now;
  toast.error(listModelsFailureToastMessage(payload.provider, payload.message));
}

/**
 * Log-friendly payload + notify UI listeners + transient toast (deduped).
 */
export function reportListModelsFallback(
  context: ListModelsFailureContext,
): ListModelsFailurePayload {
  const payload = describeListModelsFailure(context);
  notifyFallbackListeners(payload);
  toastListModelsFallback(payload);
  return payload;
}
