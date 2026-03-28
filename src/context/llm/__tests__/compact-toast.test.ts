import { describe, it, expect, vi } from 'vitest';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import type { Message, RustMessage } from '@/models/chat';

type ToastCall = {
  kind: 'loading' | 'success' | 'error';
  title: string;
  id: string;
  description: string;
  duration: number;
};

interface MockToast {
  loading: (title: string, opts: { id: string; description: string; duration: number }) => void;
  success: (title: string, opts: { id: string; description: string; duration: number }) => void;
  error: (title: string, opts: { id: string; description: string; duration: number }) => void;
  calls: ToastCall[];
}

function makeToast(): MockToast {
  const calls: ToastCall[] = [];
  return {
    calls,
    loading: (title, opts) => calls.push({ kind: 'loading', title, ...opts }),
    success: (title, opts) => calls.push({ kind: 'success', title, ...opts }),
    error: (title, opts) => calls.push({ kind: 'error', title, ...opts }),
  };
}

interface CompactPayload {
  sessionId: string;
  sessionName: string;
  messages: Array<Message | RustMessage>;
  fromId: string;
  toId: string;
}

interface Settings {
  preferredModel: { provider: string; model: string };
  serviceConfigs?: Record<string, { apiKey?: string; baseUrl?: string }>;
}

interface CompactStatePayload {
  sessionId: string;
  sessionName?: string;
  compacting: boolean;
  phase: 'STARTED' | 'SUCCEEDED' | 'FAILED';
}

async function handleCompactEvent(
  payload: CompactPayload,
  settings: Settings | null,
  getService: (
    provider: string,
    apiKey: string,
    providerConfig: { apiKey?: string; baseUrl?: string },
  ) => {
    compact: (
      messages: Message[],
      opts: { modelName: string },
    ) => Promise<string>;
  },
  handleCompactResponse: (
    sessionId: string,
    fromId: string,
    toId: string,
    summary: string,
  ) => Promise<void>,
  handleCompactError: (sessionId: string, error: unknown) => Promise<void>,
  resetContextUsageForSession: (sessionId: string) => void,
  setCompactedRangeForSession: (
    sessionId: string,
    range: { fromId: string; toId: string },
  ) => void,
): Promise<void> {
  const { sessionId, messages, fromId, toId } = payload;

  if (!settings) {
    await handleCompactError(sessionId, {} as unknown);
    return;
  }

  const provider = settings.preferredModel.provider;
  const providerConfig = settings.serviceConfigs?.[provider] ?? {};
  const apiKey = providerConfig.apiKey ?? '';
  const model = settings.preferredModel.model;
  const normalizedMessages = messages.map((message) =>
    normalizeRustMessage(message),
  );

  try {
    const service = getService(provider, apiKey, providerConfig);
    const summary = await service.compact(normalizedMessages, {
      modelName: model,
    });
    await handleCompactResponse(sessionId, fromId, toId, summary);
    setCompactedRangeForSession(sessionId, { fromId, toId });
    resetContextUsageForSession(sessionId);
  } catch {
    await handleCompactError(sessionId, {} as unknown);
  }
}

function handleCompactStateEvent(payload: CompactStatePayload, toast: MockToast): void {
  const toastId = `compact-${payload.sessionId}`;
  const description = payload.sessionName ?? payload.sessionId.slice(0, 8);

  if (payload.phase === 'STARTED') {
    toast.loading('Compacting context…', {
      id: toastId,
      description,
      duration: Infinity,
    });
    return;
  }

  if (payload.phase === 'SUCCEEDED') {
    toast.success('Context compacted', {
      id: toastId,
      description,
      duration: 3000,
    });
    return;
  }

  toast.error('Compaction failed', {
    id: toastId,
    description,
    duration: 4000,
  });
}

const SESSION_ID = 'abc-123-session';
const SESSION_NAME = 'My Agent';
const FROM_ID = 'msg-001';
const TO_ID = 'msg-099';
const MESSAGES: Message[] = [
  {
    id: 'message-1',
    sessionId: SESSION_ID,
    threadId: SESSION_ID,
    role: 'user',
    content: [{ type: 'text', text: 'hello' }],
    createdAt: new Date(1),
    updatedAt: new Date(1),
  },
];

const DEFAULT_SETTINGS: Settings = {
  preferredModel: { provider: 'openai', model: 'gpt-4o' },
  serviceConfigs: { openai: { apiKey: 'sk-test' } },
};

const DEFAULT_PAYLOAD: CompactPayload = {
  sessionId: SESSION_ID,
  sessionName: SESSION_NAME,
  messages: MESSAGES,
  fromId: FROM_ID,
  toId: TO_ID,
};

describe('compact state toast flow', () => {
  it('uses compact-${sessionId} as the toast id for all phases', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: true, phase: 'STARTED' },
      toast,
    );
    handleCompactStateEvent(
      {
        sessionId: SESSION_ID,
        sessionName: SESSION_NAME,
        compacting: false,
        phase: 'SUCCEEDED',
      },
      toast,
    );
    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: false, phase: 'FAILED' },
      toast,
    );

    expect(toast.calls.every((call) => call.id === `compact-${SESSION_ID}`)).toBe(true);
  });

  it('shows loading for STARTED phase', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: true, phase: 'STARTED' },
      toast,
    );

    expect(toast.calls).toEqual([
      {
        kind: 'loading',
        title: 'Compacting context…',
        id: `compact-${SESSION_ID}`,
        description: SESSION_NAME,
        duration: Infinity,
      },
    ]);
  });

  it('shows success for SUCCEEDED phase', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      {
        sessionId: SESSION_ID,
        sessionName: SESSION_NAME,
        compacting: false,
        phase: 'SUCCEEDED',
      },
      toast,
    );

    expect(toast.calls[0]).toMatchObject({
      kind: 'success',
      title: 'Context compacted',
      id: `compact-${SESSION_ID}`,
      description: SESSION_NAME,
    });
  });

  it('shows error for FAILED phase', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: false, phase: 'FAILED' },
      toast,
    );

    expect(toast.calls[0]).toMatchObject({
      kind: 'error',
      title: 'Compaction failed',
      id: `compact-${SESSION_ID}`,
      description: SESSION_NAME,
    });
  });

  it('falls back to short session id when sessionName is missing', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      { sessionId: SESSION_ID, compacting: true, phase: 'STARTED' },
      toast,
    );

    expect(toast.calls[0].description).toBe(SESSION_ID.slice(0, 8));
  });

  it('shows loading again when joining an in-flight compaction', () => {
    const toast = makeToast();

    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: true, phase: 'STARTED' },
      toast,
    );
    handleCompactStateEvent(
      { sessionId: SESSION_ID, sessionName: SESSION_NAME, compacting: true, phase: 'STARTED' },
      toast,
    );

    expect(toast.calls).toHaveLength(2);
    expect(toast.calls[0].id).toBe(toast.calls[1].id);
    expect(toast.calls[0].kind).toBe('loading');
    expect(toast.calls[1].kind).toBe('loading');
  });
});

describe('compact request handler', () => {
  it('calls compact service with the model from settings', async () => {
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(compactService).toHaveBeenCalledWith(MESSAGES, { modelName: 'gpt-4o' });
  });

  it('normalizes Rust message tool fields before compacting', async () => {
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });
    const rustPayload: CompactPayload = {
      ...DEFAULT_PAYLOAD,
      messages: [
        {
          id: 'assistant-1',
          sessionId: SESSION_ID,
          role: 'assistant',
          content: [],
          toolCalls: [
            {
              id: 'tc-1',
              type: 'function',
              function: {
                name: 'workspace__readFile',
                arguments: '{"path":"src/foo.ts"}',
              },
            },
          ],
          createdAt: 1,
          updatedAt: 1,
        },
        {
          id: 'tool-1',
          sessionId: SESSION_ID,
          role: 'tool',
          content: [],
          toolCallId: 'tc-1',
          createdAt: 2,
          updatedAt: 2,
        },
      ],
    };

    await handleCompactEvent(
      rustPayload,
      DEFAULT_SETTINGS,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(compactService).toHaveBeenCalledWith(
      [
        expect.objectContaining({
          tool_calls: [
            expect.objectContaining({
              id: 'tc-1',
            }),
          ],
        }),
        expect.objectContaining({
          tool_call_id: 'tc-1',
        }),
      ],
      { modelName: 'gpt-4o' },
    );
  });

  it('forwards provider config into AIServiceFactory.getService', async () => {
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(getService).toHaveBeenCalledWith('openai', 'sk-test', {
      apiKey: 'sk-test',
    });
  });

  it('forwards summary to handleCompactResponse on success', async () => {
    const compactService = vi.fn().mockResolvedValue('A concise summary.');
    const getService = vi.fn().mockReturnValue({ compact: compactService });
    const handleCompactResponse = vi.fn().mockResolvedValue(undefined);

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      getService,
      handleCompactResponse,
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(handleCompactResponse).toHaveBeenCalledWith(
      SESSION_ID,
      FROM_ID,
      TO_ID,
      'A concise summary.',
    );
  });

  it('clears in-flight state via handleCompactError on failure', async () => {
    const compactService = vi.fn().mockRejectedValue(new Error('LLM rate limit'));
    const getService = vi.fn().mockReturnValue({ compact: compactService });
    const handleCompactError = vi.fn().mockResolvedValue(undefined);

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      getService,
      vi.fn().mockResolvedValue(undefined),
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );

    expect(handleCompactError).toHaveBeenCalledWith(SESSION_ID, expect.anything());
  });

  it('clears in-flight state when settings are unavailable', async () => {
    const handleCompactError = vi.fn().mockResolvedValue(undefined);

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      null,
      vi.fn(),
      vi.fn().mockResolvedValue(undefined),
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );

    expect(handleCompactError).toHaveBeenCalledWith(SESSION_ID, expect.anything());
  });
});
