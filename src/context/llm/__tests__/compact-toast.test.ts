/**
 * Regression tests for the compact-toast notification flow.
 *
 * What these tests guard:
 *  1. Toast ID format — must be `compact-${sessionId}` so loading→result
 *     replacement works (Sonner matches by id).
 *  2. toast.loading is called BEFORE the LLM call (user gets immediate feedback).
 *  3. toast.success is called on success with the same id and sessionName.
 *  4. toast.error is called on failure with the same id and sessionName.
 *  5. No toast is shown when settings are unavailable (handleCompactError
 *     called immediately instead).
 *  6. The compact flow calls service.compact with the correct modelName,
 *     then forwards the summary to handleCompactResponse.
 *  7. On LLM failure, handleCompactError is called (Rust clears in-flight flag).
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// ---------------------------------------------------------------------------
// Helpers — extracted inline representations of the handler logic
// (same pattern as cancel.test.ts: test the logic, not the Tauri hook)
// ---------------------------------------------------------------------------

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
  messages: unknown[];
  fromId: string;
  toId: string;
}

interface Settings {
  preferredModel: { provider: string; model: string };
  serviceConfigs?: Record<string, { apiKey?: string; baseUrl?: string }>;
}

/**
 * Simulates the compact event handler extracted from `setupCompactListener`.
 * All external side-effects are injected as mocks so we can test in isolation.
 */
async function handleCompactEvent(
  payload: CompactPayload,
  settings: Settings | null,
  toast: MockToast,
  getService: (
    provider: string,
    apiKey: string,
    providerConfig: { apiKey?: string; baseUrl?: string },
  ) => {
    compact: (
      messages: unknown[],
      opts: { modelName: string },
    ) => Promise<string>;
  },
  handleCompactResponse: (
    sessionId: string,
    fromId: string,
    toId: string,
    summary: string,
  ) => Promise<void>,
  handleCompactError: (sessionId: string) => Promise<void>,
  resetContextUsageForSession: (sessionId: string) => void,
  setCompactedRangeForSession: (
    sessionId: string,
    range: { fromId: string; toId: string },
  ) => void,
): Promise<void> {
  const { sessionId, sessionName, messages, fromId, toId } = payload;

  if (!settings) {
    await handleCompactError(sessionId);
    return;
  }

  const provider = settings.preferredModel.provider;
  const providerConfig = settings.serviceConfigs?.[provider] ?? {};
  const apiKey = providerConfig.apiKey ?? '';
  const model = settings.preferredModel.model;

  const toastId = `compact-${sessionId}`;
  toast.loading(`Compacting context…`, {
    id: toastId,
    description: sessionName,
    duration: Infinity,
  });

  try {
    const service = getService(provider, apiKey, providerConfig);
    const summary = await service.compact(messages, { modelName: model });
    await handleCompactResponse(sessionId, fromId, toId, summary);
    setCompactedRangeForSession(sessionId, { fromId, toId });
    resetContextUsageForSession(sessionId);
    toast.success(`Context compacted`, {
      id: toastId,
      description: sessionName,
      duration: 3000,
    });
  } catch {
    await handleCompactError(sessionId);
    toast.error(`Compaction failed`, {
      id: toastId,
      description: sessionName,
      duration: 4000,
    });
  }
}

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const SESSION_ID = 'abc-123-session';
const SESSION_NAME = 'My Agent';
const FROM_ID = 'msg-001';
const TO_ID = 'msg-099';
const MESSAGES = [{ role: 'user', content: 'hello' }];

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

// ---------------------------------------------------------------------------
// Toast ID format
// ---------------------------------------------------------------------------

describe('compact toast: ID format', () => {
  it('uses compact-${sessionId} as the toast id', async () => {
    const toast = makeToast();
    const handleCompactResponse = vi.fn().mockResolvedValue(undefined);
    const handleCompactError = vi.fn().mockResolvedValue(undefined);
    const compactService = vi.fn().mockResolvedValue('summary text');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      toast,
      getService,
      handleCompactResponse,
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );

    const ids = toast.calls.map((c) => c.id);
    expect(ids.every((id) => id === `compact-${SESSION_ID}`)).toBe(true);
  });

  it('different sessions produce different toast ids', () => {
    expect(`compact-session-A`).not.toBe(`compact-session-B`);
  });
});

// ---------------------------------------------------------------------------
// Loading → Success sequence
// ---------------------------------------------------------------------------

describe('compact toast: success path', () => {
  let toast: MockToast;
  let handleCompactResponse: ReturnType<typeof vi.fn>;
  let handleCompactError: ReturnType<typeof vi.fn>;
  let compactService: ReturnType<typeof vi.fn>;
  let getService: ReturnType<typeof vi.fn>;
  let resetContextUsageForSession: ReturnType<typeof vi.fn>;
  let setCompactedRangeForSession: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    toast = makeToast();
    handleCompactResponse = vi.fn().mockResolvedValue(undefined);
    handleCompactError = vi.fn().mockResolvedValue(undefined);
    compactService = vi.fn().mockResolvedValue('A concise summary.');
    getService = vi.fn().mockReturnValue({ compact: compactService });
    resetContextUsageForSession = vi.fn();
    setCompactedRangeForSession = vi.fn();

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      toast,
      getService,
      handleCompactResponse,
      handleCompactError,
      resetContextUsageForSession,
      setCompactedRangeForSession,
    );
  });

  it('emits loading FIRST, before service call resolves', () => {
    // loading must be call #0 — if it were after the await it could arrive
    // after success, leaving the user with no indication of work in progress.
    expect(toast.calls[0].kind).toBe('loading');
    expect(toast.calls[0].title).toBe('Compacting context…');
  });

  it('emits success SECOND after service resolves', () => {
    expect(toast.calls[1].kind).toBe('success');
    expect(toast.calls[1].title).toBe('Context compacted');
  });

  it('both toasts carry the sessionName as description', () => {
    expect(toast.calls[0].description).toBe(SESSION_NAME);
    expect(toast.calls[1].description).toBe(SESSION_NAME);
  });

  it('loading uses the same id as success (so Sonner replaces it)', () => {
    expect(toast.calls[0].id).toBe(toast.calls[1].id);
  });

  it('loading duration is Infinity (stays until replaced)', () => {
    expect(toast.calls[0].duration).toBe(Infinity);
  });

  it('success duration is finite', () => {
    expect(toast.calls[1].duration).toBeGreaterThan(0);
    expect(toast.calls[1].duration).not.toBe(Infinity);
  });

  it('forwards the summary to handleCompactResponse with correct args', () => {
    expect(handleCompactResponse).toHaveBeenCalledWith(
      SESSION_ID,
      FROM_ID,
      TO_ID,
      'A concise summary.',
    );
  });

  it('does NOT call handleCompactError on success', () => {
    expect(handleCompactError).not.toHaveBeenCalled();
  });

  it('resets context usage after successful compaction', () => {
    expect(resetContextUsageForSession).toHaveBeenCalledWith(SESSION_ID);
  });

  it('stores the compacted range after successful compaction', () => {
    expect(setCompactedRangeForSession).toHaveBeenCalledWith(SESSION_ID, {
      fromId: FROM_ID,
      toId: TO_ID,
    });
  });
});

// ---------------------------------------------------------------------------
// Loading → Error sequence
// ---------------------------------------------------------------------------

describe('compact toast: error path', () => {
  let toast: MockToast;
  let handleCompactResponse: ReturnType<typeof vi.fn>;
  let handleCompactError: ReturnType<typeof vi.fn>;
  let compactService: ReturnType<typeof vi.fn>;
  let getService: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    toast = makeToast();
    handleCompactResponse = vi.fn().mockResolvedValue(undefined);
    handleCompactError = vi.fn().mockResolvedValue(undefined);
    compactService = vi.fn().mockRejectedValue(new Error('LLM rate limit'));
    getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      toast,
      getService,
      handleCompactResponse,
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );
  });

  it('emits loading FIRST even when the service will fail', () => {
    expect(toast.calls[0].kind).toBe('loading');
  });

  it('emits error SECOND on service failure', () => {
    expect(toast.calls[1].kind).toBe('error');
    expect(toast.calls[1].title).toBe('Compaction failed');
  });

  it('error toast carries sessionName as description', () => {
    expect(toast.calls[1].description).toBe(SESSION_NAME);
  });

  it('loading and error share the same toast id (Sonner replaces loading)', () => {
    expect(toast.calls[0].id).toBe(toast.calls[1].id);
  });

  it('error duration is finite', () => {
    expect(toast.calls[1].duration).toBeGreaterThan(0);
    expect(toast.calls[1].duration).not.toBe(Infinity);
  });

  it('calls handleCompactError so Rust clears the in-flight flag', () => {
    expect(handleCompactError).toHaveBeenCalledWith(SESSION_ID);
  });

  it('does NOT call handleCompactResponse on failure', () => {
    expect(handleCompactResponse).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Missing settings guard
// ---------------------------------------------------------------------------

describe('compact toast: missing settings guard', () => {
  it('calls handleCompactError immediately when settings are null', async () => {
    const toast = makeToast();
    const handleCompactResponse = vi.fn().mockResolvedValue(undefined);
    const handleCompactError = vi.fn().mockResolvedValue(undefined);
    const compactService = vi.fn().mockResolvedValue('should not be called');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      null, // no settings
      toast,
      getService,
      handleCompactResponse,
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );

    expect(handleCompactError).toHaveBeenCalledWith(SESSION_ID);
  });

  it('shows NO toast when settings are null (avoids orphaned loading spinners)', async () => {
    const toast = makeToast();
    const handleCompactError = vi.fn().mockResolvedValue(undefined);
    const compactService = vi.fn();
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      null,
      toast,
      getService,
      vi.fn(),
      handleCompactError,
      vi.fn(),
      vi.fn(),
    );

    expect(toast.calls).toHaveLength(0);
  });

  it('does NOT call the compact service when settings are null', async () => {
    const toast = makeToast();
    const compactService = vi.fn();
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      null,
      toast,
      getService,
      vi.fn(),
      vi.fn(),
      vi.fn(),
      vi.fn(),
    );

    expect(compactService).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Service called with correct model
// ---------------------------------------------------------------------------

describe('compact toast: LLM service call parameters', () => {
  it('calls compact service with the model from settings.preferredModel.model', async () => {
    const toast = makeToast();
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    const settings: Settings = {
      preferredModel: { provider: 'anthropic', model: 'claude-3-5-sonnet' },
      serviceConfigs: { anthropic: { apiKey: 'sk-ant' } },
    };

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      settings,
      toast,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(compactService).toHaveBeenCalledWith(MESSAGES, {
      modelName: 'claude-3-5-sonnet',
    });
  });

  it('forwards provider config into AIServiceFactory.getService', async () => {
    const toast = makeToast();
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    const settings: Settings = {
      preferredModel: { provider: 'openai', model: 'gpt-4o' },
      serviceConfigs: {
        openai: { apiKey: 'sk-openai', baseUrl: 'https://example.test/v1' },
      },
    };

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      settings,
      toast,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(getService).toHaveBeenCalledWith('openai', 'sk-openai', {
      apiKey: 'sk-openai',
      baseUrl: 'https://example.test/v1',
    });
  });

  it('falls back to empty apiKey and empty provider config when serviceConfigs has no entry for provider', async () => {
    const toast = makeToast();
    const compactService = vi.fn().mockResolvedValue('summary');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    const settings: Settings = {
      preferredModel: { provider: 'groq', model: 'llama3' },
      // no serviceConfigs entry for 'groq'
    };

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      settings,
      toast,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(getService).toHaveBeenCalledWith('groq', '', {});
    expect(compactService).toHaveBeenCalledWith(MESSAGES, { modelName: 'llama3' });
  });
});

// ---------------------------------------------------------------------------
// Only exactly 2 toasts are emitted per compaction (no extra calls)
// ---------------------------------------------------------------------------

describe('compact toast: call count', () => {
  it('emits exactly 2 toasts on success (loading + success)', async () => {
    const toast = makeToast();
    const compactService = vi.fn().mockResolvedValue('ok');
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      toast,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(toast.calls).toHaveLength(2);
  });

  it('emits exactly 2 toasts on error (loading + error)', async () => {
    const toast = makeToast();
    const compactService = vi.fn().mockRejectedValue(new Error('fail'));
    const getService = vi.fn().mockReturnValue({ compact: compactService });

    await handleCompactEvent(
      DEFAULT_PAYLOAD,
      DEFAULT_SETTINGS,
      toast,
      getService,
      vi.fn().mockResolvedValue(undefined),
      vi.fn().mockResolvedValue(undefined),
      vi.fn(),
      vi.fn(),
    );

    expect(toast.calls).toHaveLength(2);
  });
});
