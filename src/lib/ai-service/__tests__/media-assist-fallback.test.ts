import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';
import { AIServiceError, AIServiceProvider } from '../types';
import {
  buildMediaAssistPlaceholder,
  localMediaPathForPlugin,
  messagesHaveMultimodalParts,
  prepareMessagesForMediaAssistRetry,
  shouldAttemptMediaAssistFallback,
  stripMultimodalFromMessages,
} from '../media-assist-fallback';

const getMediaAssistPluginStatus = vi.fn();
const runMediaAssistPlugin = vi.fn();

vi.mock('../media-assist-host', () => ({
  getMediaAssistPluginStatus: (...args: unknown[]) =>
    getMediaAssistPluginStatus(...args),
  runMediaAssistPlugin: (...args: unknown[]) => runMediaAssistPlugin(...args),
}));

function userWithAudio(): Message {
  return {
    id: 'm1',
    sessionId: 's1',
    threadId: 's1',
    role: 'user',
    content: [
      { type: 'text', text: 'transcribe this' },
      { type: 'audio', data: 'abc', mimeType: 'audio/wav' },
    ],
    createdAt: new Date(),
  };
}

describe('media-assist-fallback', () => {
  it('detects multimodal parts in messages', () => {
    expect(messagesHaveMultimodalParts([userWithAudio()])).toBe(true);
    expect(
      messagesHaveMultimodalParts([
        {
          ...userWithAudio(),
          content: [{ type: 'text', text: 'plain text' }],
        },
      ]),
    ).toBe(false);
  });

  it('strips audio/image/video and injects skill placeholder', () => {
    const input: Message = {
      id: 'm2',
      sessionId: 's1',
      threadId: 's1',
      role: 'user',
      content: [
        { type: 'text', text: 'look' },
        { type: 'image', data: 'img', mimeType: 'image/png' },
        { type: 'audio', data: 'aud', mimeType: 'audio/mp3' },
        { type: 'video', data: 'vid', mimeType: 'video/mp4' },
      ],
      createdAt: new Date(),
    };

    const [out] = stripMultimodalFromMessages([input]);
    expect(Array.isArray(out.content)).toBe(true);
    if (!Array.isArray(out.content)) {
      throw new Error('expected array content');
    }
    expect(out.content.some((p) => p.type === 'image')).toBe(false);
    expect(out.content.some((p) => p.type === 'audio')).toBe(false);
    expect(out.content.some((p) => p.type === 'video')).toBe(false);
    const placeholder = out.content.find((p) => p.type === 'text' && p.text.includes('[media-assist]'));
    expect(placeholder?.type).toBe('text');
    if (placeholder?.type === 'text') {
      expect(placeholder.text).toContain('@skill:libragent-plugin');
      expect(placeholder.text).toContain('audio/image/video');
    }
  });

  it('builds placeholder listing stripped kinds', () => {
    const text = buildMediaAssistPlaceholder({
      audio: true,
      image: false,
      video: false,
    });
    expect(text).toContain('audio');
    expect(text).toContain('@skill:libragent-plugin');
  });

  it('accepts only local media paths for the host plugin', () => {
    expect(localMediaPathForPlugin('/tmp/a.wav')).toBe('/tmp/a.wav');
    expect(localMediaPathForPlugin('file:///tmp/a.wav')).toBe('file:///tmp/a.wav');
    expect(localMediaPathForPlugin('https://example.com/a.wav')).toBeUndefined();
    expect(localMediaPathForPlugin('data:audio/wav;base64,abc')).toBeUndefined();
  });

  it('attempts fallback on 400 with multimodal error hint when recent turn has media', () => {
    const error = new AIServiceError(
      'Bad Request: audio modality is unsupported',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    expect(shouldAttemptMediaAssistFallback(error, [userWithAudio()])).toBe(
      true,
    );
  });

  it('does not attempt fallback on bare 400 without multimodal hint', () => {
    const error = new AIServiceError(
      'Bad Request: invalid tool schema',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    expect(shouldAttemptMediaAssistFallback(error, [userWithAudio()])).toBe(
      false,
    );
  });

  it('attempts fallback when older turns still have unstripped media', () => {
    const error = new AIServiceError(
      'Bad Request: audio modality is unsupported',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    const assistant: Message = {
      ...userWithAudio(),
      id: 'm-assistant',
      role: 'assistant',
      content: [{ type: 'text', text: 'ok' }],
    };
    const laterTextOnly: Message = {
      ...userWithAudio(),
      id: 'm-later',
      content: [{ type: 'text', text: 'follow up without media' }],
    };
    // Turn-2 poison: prior turn's media remains in history after in-memory-only strip.
    expect(
      shouldAttemptMediaAssistFallback(error, [
        userWithAudio(),
        assistant,
        laterTextOnly,
      ]),
    ).toBe(true);
  });

  it('detects multimodal parts across multi-tool turns', () => {
    const error = new AIServiceError(
      'Bad Request: audio modality is unsupported',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    const toolAudio: Message = {
      ...userWithAudio(),
      id: 'tool-audio',
      role: 'tool',
      content: [
        { type: 'audio', data: 'abc', mimeType: 'audio/wav' },
      ],
    };
    const toolText: Message = {
      ...userWithAudio(),
      id: 'tool-text',
      role: 'tool',
      content: [{ type: 'text', text: 'wrote file' }],
    };
    expect(
      shouldAttemptMediaAssistFallback(error, [toolAudio, toolText]),
    ).toBe(true);
  });

  it('does not attempt fallback on generic Error with 429/5xx text', () => {
    expect(
      shouldAttemptMediaAssistFallback(
        new Error('429 Rate limit: too many audio requests'),
        [userWithAudio()],
      ),
    ).toBe(false);
    expect(
      shouldAttemptMediaAssistFallback(
        new Error('500 Internal Server Error: failed to process video'),
        [userWithAudio()],
      ),
    ).toBe(false);
  });

  it('does not attempt fallback on generic Error with statusCode 500', () => {
    const err = Object.assign(new Error('failed to process video'), {
      statusCode: 500,
    });
    expect(shouldAttemptMediaAssistFallback(err, [userWithAudio()])).toBe(
      false,
    );
  });

  it('does not attempt fallback on 429/500 even with multimodal hints', () => {
    const rateLimit = new AIServiceError(
      'Rate limit: too many image requests',
      AIServiceProvider.OpenAI,
      429,
    );
    const serverError = new AIServiceError(
      'Internal error while processing audio',
      AIServiceProvider.OpenAI,
      500,
    );
    expect(shouldAttemptMediaAssistFallback(rateLimit, [userWithAudio()])).toBe(
      false,
    );
    expect(
      shouldAttemptMediaAssistFallback(serverError, [userWithAudio()]),
    ).toBe(false);
  });

  it('does not match generic does-not-support without media modality', () => {
    const error = new AIServiceError(
      'Bad Request: model does not support tools',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    expect(shouldAttemptMediaAssistFallback(error, [userWithAudio()])).toBe(
      false,
    );
  });

  it('attempts fallback on 415 when recent turn has media', () => {
    const error = new AIServiceError(
      'Unsupported Media Type',
      AIServiceProvider.OpenAI,
      415,
    );
    expect(shouldAttemptMediaAssistFallback(error, [userWithAudio()])).toBe(
      true,
    );
  });

  it('does not attempt fallback without multimodal parts', () => {
    const error = new AIServiceError(
      'Bad Request: audio modality is unsupported',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'invalid_request' },
    );
    expect(
      shouldAttemptMediaAssistFallback(error, [
        { ...userWithAudio(), content: [{ type: 'text', text: 'no media' }] },
      ]),
    ).toBe(false);
  });

  it('does not attempt fallback for context_limit errors', () => {
    const error = new AIServiceError(
      'Prompt too long',
      AIServiceProvider.OpenAI,
      400,
      undefined,
      { kind: 'context_limit' },
    );
    expect(shouldAttemptMediaAssistFallback(error, [userWithAudio()])).toBe(
      false,
    );
  });

  describe('prepareMessagesForMediaAssistRetry', () => {
    beforeEach(() => {
      getMediaAssistPluginStatus.mockReset();
      runMediaAssistPlugin.mockReset();
    });

    it('strips when plugin is not installed', async () => {
      getMediaAssistPluginStatus.mockResolvedValue({
        installed: false,
        path: '/tmp/plugin',
        modalities: [],
        timeoutMs: 1000,
      });

      const { messages, usedPlugin } = await prepareMessagesForMediaAssistRetry([
        userWithAudio(),
      ]);
      expect(usedPlugin).toBe(false);
      expect(runMediaAssistPlugin).not.toHaveBeenCalled();
      const content = messages[0]?.content;
      expect(Array.isArray(content)).toBe(true);
      if (!Array.isArray(content)) {
        throw new Error('expected array content');
      }
      expect(content.some((p) => p.type === 'audio')).toBe(false);
      expect(
        content.some(
          (p) => p.type === 'text' && p.text.includes('@skill:libragent-plugin'),
        ),
      ).toBe(true);
    });

    it('converts media via plugin when installed', async () => {
      getMediaAssistPluginStatus.mockResolvedValue({
        installed: true,
        path: '/tmp/plugin',
        modalities: ['audio'],
        timeoutMs: 1000,
      });
      runMediaAssistPlugin.mockResolvedValue({
        ok: true,
        text: 'hello from asr',
        notes: 'stub',
      });

      const { messages, usedPlugin } = await prepareMessagesForMediaAssistRetry([
        userWithAudio(),
      ]);
      expect(usedPlugin).toBe(true);
      expect(runMediaAssistPlugin).toHaveBeenCalledOnce();
      const content = messages[0]?.content;
      expect(Array.isArray(content)).toBe(true);
      if (!Array.isArray(content)) {
        throw new Error('expected array content');
      }
      expect(content.some((p) => p.type === 'audio')).toBe(false);
      const converted = content.find(
        (p) => p.type === 'text' && p.text.includes('[media-assist:audio]'),
      );
      expect(converted?.type).toBe('text');
      if (converted?.type === 'text') {
        expect(converted.text).toContain('hello from asr');
      }
    });

    it('falls back to strip when plugin run fails', async () => {
      getMediaAssistPluginStatus.mockResolvedValue({
        installed: true,
        path: '/tmp/plugin',
        modalities: ['audio'],
        timeoutMs: 1000,
      });
      runMediaAssistPlugin.mockResolvedValue({
        ok: false,
        error: 'exec_failed',
      });

      const { messages, usedPlugin } = await prepareMessagesForMediaAssistRetry([
        userWithAudio(),
      ]);
      expect(usedPlugin).toBe(false);
      const content = messages[0]?.content;
      expect(Array.isArray(content)).toBe(true);
      if (!Array.isArray(content)) {
        throw new Error('expected array content');
      }
      expect(
        content.some(
          (p) => p.type === 'text' && p.text.includes('@skill:libragent-plugin'),
        ),
      ).toBe(true);
    });
  });
});
