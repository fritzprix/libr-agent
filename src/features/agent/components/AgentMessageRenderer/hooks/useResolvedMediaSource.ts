import { useEffect, useState } from 'react';
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';
import { getLogger } from '@/lib/logger';
import {
  estimateBase64Bytes,
  getDisplayMediaCacheEntry,
  touchDisplayMediaCacheEntry,
  updateDisplayMediaCache,
} from '../utils/displayMediaCache';

const logger = getLogger('AgentMessageRenderer');

function inlineMediaToDataUrl(rawData: string, mimeType: string): string {
  return rawData.startsWith('data:')
    ? rawData
    : `data:${mimeType};base64,${rawData}`;
}

export function useResolvedMediaSource(
  rawData: string | undefined,
  uri: string | undefined,
  mimeType: string,
  sessionId: string | undefined,
): { resolvedSrc: string | undefined; loadError?: string } {
  const [resolvedSrc, setResolvedSrc] = useState<string | undefined>(() => {
    if (rawData) {
      return inlineMediaToDataUrl(rawData, mimeType);
    }

    if (!uri) {
      return undefined;
    }

    if (!uri.startsWith('file://')) {
      return uri;
    }

    return sessionId
      ? getDisplayMediaCacheEntry(sessionId, uri)?.url
      : undefined;
  });
  const [loadError, setLoadError] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;

    if (rawData) {
      setResolvedSrc(inlineMediaToDataUrl(rawData, mimeType));
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    if (!uri) {
      setResolvedSrc(undefined);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    if (!uri.startsWith('file://')) {
      setResolvedSrc(uri);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    const cached = sessionId
      ? touchDisplayMediaCacheEntry(sessionId, uri)
      : undefined;
    if (cached) {
      setResolvedSrc(cached.url);
      setLoadError(undefined);
      return () => {
        cancelled = true;
      };
    }

    setResolvedSrc(undefined);

    if (!sessionId) {
      const errorMessage = 'Cannot read local media without a session';
      logger.error('Failed to resolve display media source', {
        uri,
        mimeType,
        error: errorMessage,
      });
      setLoadError(errorMessage);
      return () => {
        cancelled = true;
      };
    }

    const activeSessionId = sessionId;

    void readLocalFileAsBase64(activeSessionId, uri)
      .then((base64) => {
        if (cancelled) {
          return;
        }

        const url = inlineMediaToDataUrl(base64, mimeType);
        updateDisplayMediaCache(
          activeSessionId,
          uri,
          url,
          estimateBase64Bytes(base64),
        );
        setResolvedSrc(url);
        setLoadError(undefined);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }

        const errorMessage =
          error instanceof Error ? error.message : String(error);
        logger.error('Failed to resolve display media source', {
          uri,
          mimeType,
          error: errorMessage,
        });
        setLoadError(errorMessage);
      });

    return () => {
      cancelled = true;
    };
  }, [mimeType, rawData, sessionId, uri]);

  return { resolvedSrc, loadError };
}
