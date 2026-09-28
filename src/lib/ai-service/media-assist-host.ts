/**
 * Host bridge for MediaAssist plugins via Tauri (#1926).
 * Gracefully no-ops outside the Tauri desktop runtime.
 *
 * Always pass `sessionId` when calling from an agent turn so Docker/Harbor
 * isolation can block host plugin ACE (matches MCP deploy/status policy).
 */
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';

const logger = getLogger('media-assist-host');

export type MediaAssistModality = 'audio' | 'image' | 'video';

export type MediaAssistPluginStatus = {
  installed: boolean;
  path: string;
  modalities: string[];
  timeoutMs: number;
  error?: string | null;
};

export type MediaAssistRunRequest = {
  modality: MediaAssistModality;
  mimeType: string;
  path?: string;
  dataBase64?: string;
  maxOutputChars?: number;
  /** Agent session — required to enforce Docker isolation on the host bridge. */
  sessionId?: string;
};

export type MediaAssistRunResponse = {
  ok: boolean;
  modality?: string;
  text?: string;
  error?: string;
  message?: string;
  notes?: string;
};

function isTauriRuntime(): boolean {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  );
}

export async function getMediaAssistPluginStatus(
  sessionId?: string,
): Promise<MediaAssistPluginStatus | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  try {
    return await safeInvoke<MediaAssistPluginStatus>(
      'media_assist_plugin_status',
      { sessionId: sessionId ?? null },
      { shouldSuppressErrorLogging: () => true },
    );
  } catch (error) {
    logger.debug('media_assist_plugin_status unavailable', { error });
    return null;
  }
}

export async function runMediaAssistPlugin(
  request: MediaAssistRunRequest,
): Promise<MediaAssistRunResponse | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  try {
    const { sessionId, ...runRequest } = request;
    return await safeInvoke<MediaAssistRunResponse>('media_assist_run_plugin', {
      request: runRequest,
      sessionId: sessionId ?? null,
    });
  } catch (error) {
    logger.warn('media_assist_run_plugin failed', { error });
    return {
      ok: false,
      error: 'invoke_failed',
      message: error instanceof Error ? error.message : String(error),
    };
  }
}
