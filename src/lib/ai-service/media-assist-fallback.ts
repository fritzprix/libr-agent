/**
 * Media-assist fallback for multimodal rejection (HTTP 400 / invalid_request).
 *
 * When a completion fails after the request included image/audio/video parts:
 * 1. Try host MediaAssist plugin (app_data_dir) to convert media → text
 * 2. Else strip media and inject @skill:libragent-plugin placeholder
 * Then allow exactly one streamChat retry.
 *
 * @see https://github.com/fritzprix/libr-agent/issues/1926
 */
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import {
  getMediaAssistPluginStatus,
  runMediaAssistPlugin,
  type MediaAssistModality,
} from './media-assist-host';
import { AIServiceError } from './types';

export const MEDIA_ASSIST_SKILL_NAME = 'libragent-plugin';

const MEDIA_TYPES = new Set(['image', 'audio', 'video']);

const MULTIMODAL_ERROR_HINT =
  /\b(audio|image|video|modality|modalities|input_audio|multimodal|unsupported\s+(?:media|content|modality)|invalid.*(?:image|audio|video)|does not support\s+(?:audio|image|video|media|modalit\w*))\b/i;

export type StrippedModalities = {
  image: boolean;
  audio: boolean;
  video: boolean;
};

export function isMultimodalContentPart(
  part: MCPContent | null | undefined,
): part is Extract<MCPContent, { type: 'image' | 'audio' | 'video' }> {
  return Boolean(part && MEDIA_TYPES.has(part.type));
}

export function contentHasMultimodalParts(
  content: Message['content'],
): boolean {
  if (!Array.isArray(content)) {
    return false;
  }
  return content.some((part) => isMultimodalContentPart(part));
}

export function messagesHaveMultimodalParts(messages: Message[]): boolean {
  return messages.some((message) => contentHasMultimodalParts(message.content));
}

export function collectStrippedModalities(
  content: Message['content'],
): StrippedModalities {
  const result: StrippedModalities = {
    image: false,
    audio: false,
    video: false,
  };
  if (!Array.isArray(content)) {
    return result;
  }
  for (const part of content) {
    if (part.type === 'image') result.image = true;
    if (part.type === 'audio') result.audio = true;
    if (part.type === 'video') result.video = true;
  }
  return result;
}

export function buildMediaAssistPlaceholder(
  stripped: StrippedModalities,
): string {
  const kinds = (
    [
      stripped.audio ? 'audio' : null,
      stripped.image ? 'image' : null,
      stripped.video ? 'video' : null,
    ] as const
  ).filter((value): value is 'audio' | 'image' | 'video' => value !== null);

  const kindList = kinds.length > 0 ? kinds.join('/') : 'media';

  return [
    `[media-assist] Multimodal ${kindList} was stripped because this model/runtime rejected it (likely HTTP 400 after media content).`,
    'Continue the task with external commands or sparse samples if needed (e.g. ffmpeg keyframes, OCR CLI). Do not spend the session apt/pip-installing ASR/OCR stacks.',
    `For a durable fix on this machine: load @skill:${MEDIA_ASSIST_SKILL_NAME}, implement a MediaAssist plugin (audio|image|video → text), run its verify script, then deploy to the harness plugin path. Once deployed, LibrAgent will auto-convert media on future modality failures.`,
  ].join('\n');
}

/**
 * Replace multimodal parts with a single text placeholder. Non-media parts kept.
 */
export function stripMultimodalContent(
  content: Message['content'],
): Message['content'] {
  if (!Array.isArray(content)) {
    return content;
  }

  const stripped = collectStrippedModalities(content);
  if (!stripped.image && !stripped.audio && !stripped.video) {
    return content;
  }

  const kept: MCPContent[] = content.filter(
    (part) => !isMultimodalContentPart(part),
  );
  kept.push({
    type: 'text',
    text: buildMediaAssistPlaceholder(stripped),
  });
  return kept;
}

export function stripMultimodalFromMessages(messages: Message[]): Message[] {
  return messages.map((message) => {
    if (!contentHasMultimodalParts(message.content)) {
      return message;
    }
    return {
      ...message,
      content: stripMultimodalContent(message.content),
    };
  });
}

function errorTextBlob(error: unknown): string {
  if (error instanceof AIServiceError) {
    const raw =
      typeof error.metadata.rawPayload === 'string'
        ? error.metadata.rawPayload
        : error.metadata.rawPayload
          ? JSON.stringify(error.metadata.rawPayload)
          : '';
    return [
      error.message,
      error.metadata.providerCode ?? '',
      error.metadata.providerStatus ?? '',
      raw,
    ].join(' ');
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/** Best-effort HTTP status from AIServiceError or SDK-shaped errors. */
function readHttpStatus(error: unknown): number | undefined {
  if (error instanceof AIServiceError && typeof error.statusCode === 'number') {
    return error.statusCode;
  }
  if (typeof error !== 'object' || error === null) {
    return undefined;
  }
  if (
    'statusCode' in error &&
    typeof (error as { statusCode: unknown }).statusCode === 'number'
  ) {
    return (error as { statusCode: number }).statusCode;
  }
  if (
    'status' in error &&
    typeof (error as { status: unknown }).status === 'number'
  ) {
    return (error as { status: number }).status;
  }
  return undefined;
}

function isRateLimitOrServerFailure(error: unknown, blob: string): boolean {
  const status = readHttpStatus(error);
  if (status === 429 || (typeof status === 'number' && status >= 500)) {
    return true;
  }
  return /\b(429|5\d{2}|rate\s*limit|quota\s*exceeded)\b/i.test(blob);
}

/**
 * True when we should strip/convert media and retry once.
 * Requires multimodal parts anywhere in the request history (not only the latest
 * user/tool turn) and a 400-class rejection (400 / 415 / 422 / invalid_request)
 * with either 415 or a multimodal error hint.
 *
 * History-wide scanning matters because a prior streamChat may have stripped
 * media only in the ephemeral retry slice; the next turn reloads unstripped
 * history from the store, and a recent-turn-only scan would stop at the
 * intervening assistant message.
 *
 * Context-limit / 429 / 5xx are never treated as media-assist.
 */
export function shouldAttemptMediaAssistFallback(
  error: unknown,
  messages: Message[],
): boolean {
  if (!messagesHaveMultimodalParts(messages)) {
    return false;
  }

  const blob = errorTextBlob(error);
  if (/\bcontext\b.*\b(length|window|exceed)/i.test(blob)) {
    return false;
  }

  // Reject rate-limit / server failures for both AIServiceError and generic Errors.
  if (isRateLimitOrServerFailure(error, blob)) {
    return false;
  }

  if (!(error instanceof AIServiceError)) {
    return MULTIMODAL_ERROR_HINT.test(blob);
  }

  if (error.metadata.kind === 'context_limit') {
    return false;
  }

  const isClientReject =
    error.statusCode === 400 ||
    error.statusCode === 415 ||
    error.statusCode === 422 ||
    error.metadata.kind === 'invalid_request';

  if (!isClientReject) {
    return false;
  }

  // 415 is unambiguously unsupported media type.
  if (error.statusCode === 415) {
    return true;
  }

  return MULTIMODAL_ERROR_HINT.test(blob);
}

function partModality(part: MCPContent): MediaAssistModality | null {
  if (part.type === 'audio' || part.type === 'image' || part.type === 'video') {
    return part.type;
  }
  return null;
}

/**
 * Only local filesystem paths / file: URLs may be passed to the host plugin.
 * Remote http(s)/data URIs are ignored (use dataBase64 when present instead).
 */
export function localMediaPathForPlugin(
  uri: string | undefined,
): string | undefined {
  if (!uri) {
    return undefined;
  }
  const trimmed = uri.trim();
  if (!trimmed) {
    return undefined;
  }
  if (/^(https?|data|blob):/i.test(trimmed)) {
    return undefined;
  }
  return trimmed;
}

function sessionIdFromMessages(messages: Message[]): string | undefined {
  for (const message of messages) {
    const id = message.sessionId?.trim();
    if (id) {
      return id;
    }
  }
  return undefined;
}

async function convertMultimodalPart(
  part: Extract<MCPContent, { type: 'image' | 'audio' | 'video' }>,
  sessionId: string | undefined,
): Promise<string | null> {
  const modality = partModality(part);
  if (!modality) {
    return null;
  }
  const mimeType =
    part.mimeType ||
    part.source?.mimeType ||
    (modality === 'audio'
      ? 'audio/wav'
      : modality === 'image'
        ? 'image/png'
        : 'video/mp4');
  const path = localMediaPathForPlugin(part.uri || part.source?.uri);
  const dataBase64 = part.data || part.source?.data;
  if (!path && !dataBase64) {
    return null;
  }
  const result = await runMediaAssistPlugin({
    modality,
    mimeType,
    path,
    dataBase64: dataBase64 || undefined,
    sessionId,
  });
  if (!result?.ok || !result.text?.trim()) {
    return null;
  }
  const note = result.notes ? ` (${result.notes})` : '';
  return `[media-assist:${modality}]${note}\n${result.text.trim()}`;
}

/**
 * Prefer host plugin conversion; fall back to strip + skill placeholder.
 */
export async function prepareMessagesForMediaAssistRetry(
  messages: Message[],
  sessionId?: string,
): Promise<{ messages: Message[]; usedPlugin: boolean }> {
  const effectiveSessionId = sessionId ?? sessionIdFromMessages(messages);
  const status = await getMediaAssistPluginStatus(effectiveSessionId);
  if (!status?.installed) {
    return {
      messages: stripMultimodalFromMessages(messages),
      usedPlugin: false,
    };
  }

  let usedPlugin = false;
  const nextMessages: Message[] = [];

  for (const message of messages) {
    if (!contentHasMultimodalParts(message.content) || !Array.isArray(message.content)) {
      nextMessages.push(message);
      continue;
    }

    const nextContent: MCPContent[] = [];
    const failedStrip: StrippedModalities = {
      image: false,
      audio: false,
      video: false,
    };

    for (const part of message.content) {
      if (!isMultimodalContentPart(part)) {
        nextContent.push(part);
        continue;
      }
      const text = await convertMultimodalPart(part, effectiveSessionId);
      if (text) {
        usedPlugin = true;
        nextContent.push({ type: 'text', text });
      } else if (part.type === 'audio') {
        failedStrip.audio = true;
      } else if (part.type === 'image') {
        failedStrip.image = true;
      } else if (part.type === 'video') {
        failedStrip.video = true;
      }
    }

    if (failedStrip.audio || failedStrip.image || failedStrip.video) {
      nextContent.push({
        type: 'text',
        text: buildMediaAssistPlaceholder(failedStrip),
      });
    }

    nextMessages.push({ ...message, content: nextContent });
  }

  if (!usedPlugin) {
    return {
      messages: stripMultimodalFromMessages(messages),
      usedPlugin: false,
    };
  }

  return { messages: nextMessages, usedPlugin: true };
}
