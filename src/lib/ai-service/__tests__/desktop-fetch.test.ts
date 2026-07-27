import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-http', () => ({
  fetch: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

import { fetch as tauriPluginFetch } from '@tauri-apps/plugin-http';
import {
  clearLlmFetchTransportPreferences,
  createLlmFetch,
  getLlmFetchTransportPreference,
  isLlmTransportNetworkError,
  isTauriLlmFetchRuntime,
  resolveLlmFetchOrigin,
} from '../desktop-fetch';

describe('desktop-fetch', () => {
  afterEach(() => {
    clearLlmFetchTransportPreferences();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('isTauriLlmFetchRuntime is false in test/Vitest env', () => {
    expect(
      isTauriLlmFetchRuntime({
        MODE: 'test',
        DEV: true,
        PROD: false,
        SSR: false,
        BASE_URL: '/',
      } as ImportMetaEnv),
    ).toBe(false);
  });

  it('resolveLlmFetchOrigin supports string, URL, and Request', () => {
    expect(resolveLlmFetchOrigin('https://example.com/v1/models')).toBe(
      'https://example.com',
    );
    expect(resolveLlmFetchOrigin(new URL('http://10.0.0.1:4444/v1'))).toBe(
      'http://10.0.0.1:4444',
    );
    expect(
      resolveLlmFetchOrigin(new Request('https://api.openai.com/v1/models')),
    ).toBe('https://api.openai.com');
  });

  it('isLlmTransportNetworkError detects connectivity failures only', () => {
    expect(isLlmTransportNetworkError(new Error('Connection error.'))).toBe(
      true,
    );
    expect(isLlmTransportNetworkError(new Error('Operation timed out'))).toBe(
      true,
    );
    expect(
      isLlmTransportNetworkError(
        new Error('error sending request for url (http://x): timed out'),
      ),
    ).toBe(true);
    expect(isLlmTransportNetworkError(new Error('Failed to fetch'))).toBe(true);
    expect(isLlmTransportNetworkError(new Error('Unauthorized'))).toBe(false);
    expect(
      isLlmTransportNetworkError(
        Object.assign(new Error('The operation was aborted.'), {
          name: 'AbortError',
        }),
      ),
    ).toBe(false);
  });

  it('createLlmFetch returns globalThis.fetch outside Tauri', async () => {
    const browserFetch = vi.fn().mockResolvedValue(new Response('ok'));
    vi.stubGlobal('fetch', browserFetch);

    const llmFetch = createLlmFetch({ isTauri: false });
    await llmFetch('https://example.com/v1/models');

    expect(browserFetch).toHaveBeenCalledWith('https://example.com/v1/models');
    expect(tauriPluginFetch).not.toHaveBeenCalled();
  });

  it('createLlmFetch uses plugin-http when isTauri is true', async () => {
    vi.mocked(tauriPluginFetch).mockResolvedValue(new Response('ok'));

    const llmFetch = createLlmFetch({ isTauri: true });
    await llmFetch('https://integrate.api.nvidia.com/v1/models', {
      method: 'GET',
    });

    expect(tauriPluginFetch).toHaveBeenCalledWith(
      'https://integrate.api.nvidia.com/v1/models',
      { method: 'GET' },
    );
    expect(
      getLlmFetchTransportPreference('https://integrate.api.nvidia.com'),
    ).toBe('native');
  });

  it('createLlmFetch forwards URL and Request inputs to plugin-http', async () => {
    vi.mocked(tauriPluginFetch).mockResolvedValue(new Response('ok'));
    const llmFetch = createLlmFetch({ isTauri: true });

    const url = new URL('https://example.com/v1/models');
    await llmFetch(url);
    expect(tauriPluginFetch).toHaveBeenCalledWith(url, undefined);

    const request = new Request('https://example.com/v1/chat');
    await llmFetch(request, { method: 'POST' });
    expect(tauriPluginFetch).toHaveBeenCalledWith(
      expect.any(Request),
      { method: 'POST' },
    );
  });

  it('falls back to webview fetch on plugin-http network errors', async () => {
    const browserFetch = vi.fn().mockResolvedValue(new Response('browser-ok'));
    vi.stubGlobal('fetch', browserFetch);
    vi.mocked(tauriPluginFetch).mockRejectedValue(
      new Error('Connection error.'),
    );

    const llmFetch = createLlmFetch({ isTauri: true });
    const response = await llmFetch('http://175.115.246.118:4444/v1/models', {
      method: 'GET',
    });

    expect(await response.text()).toBe('browser-ok');
    expect(tauriPluginFetch).toHaveBeenCalledTimes(1);
    expect(browserFetch).toHaveBeenCalledWith(
      'http://175.115.246.118:4444/v1/models',
      { method: 'GET' },
    );
    expect(
      getLlmFetchTransportPreference('http://175.115.246.118:4444'),
    ).toBe('browser');
  });

  it('falls back when plugin-http hangs past native probe timeout', async () => {
    vi.useFakeTimers();
    const browserFetch = vi.fn().mockResolvedValue(new Response('browser-ok'));
    vi.stubGlobal('fetch', browserFetch);
    // Never resolves — mimics corporate PAC hang via WinHTTP/reqwest.
    vi.mocked(tauriPluginFetch).mockImplementation(
      () => new Promise<Response>(() => {}),
    );

    const llmFetch = createLlmFetch({
      isTauri: true,
      nativeProbeTimeoutMs: 100,
    });
    const pending = llmFetch('http://175.115.246.118:4444/v1/models');
    await vi.advanceTimersByTimeAsync(100);
    const response = await pending;

    expect(await response.text()).toBe('browser-ok');
    expect(browserFetch).toHaveBeenCalledTimes(1);
    expect(
      getLlmFetchTransportPreference('http://175.115.246.118:4444'),
    ).toBe('browser');

    vi.useRealTimers();
  });

  it('uses sticky browser transport on subsequent requests', async () => {
    const browserFetch = vi.fn().mockResolvedValue(new Response('ok'));
    vi.stubGlobal('fetch', browserFetch);
    vi.mocked(tauriPluginFetch).mockRejectedValue(
      new Error('Operation timed out'),
    );

    const llmFetch = createLlmFetch({ isTauri: true });
    const url = 'http://175.115.246.118:4444/v1/models';

    await llmFetch(url);
    await llmFetch(url);

    expect(tauriPluginFetch).toHaveBeenCalledTimes(1);
    expect(browserFetch).toHaveBeenCalledTimes(2);
  });

  it('does not fall back on non-network plugin-http errors', async () => {
    const browserFetch = vi.fn();
    vi.stubGlobal('fetch', browserFetch);
    vi.mocked(tauriPluginFetch).mockRejectedValue(
      new Error('url not allowed on the configured scope'),
    );

    const llmFetch = createLlmFetch({ isTauri: true });
    await expect(llmFetch('https://blocked.example/v1/models')).rejects.toThrow(
      'url not allowed on the configured scope',
    );
    expect(browserFetch).not.toHaveBeenCalled();
  });

  it('does not treat HTTP 4xx Response as a transport failure', async () => {
    const browserFetch = vi.fn();
    vi.stubGlobal('fetch', browserFetch);
    vi.mocked(tauriPluginFetch).mockResolvedValue(
      new Response('unauthorized', { status: 401 }),
    );

    const llmFetch = createLlmFetch({ isTauri: true });
    const response = await llmFetch('https://api.openai.com/v1/models');

    expect(response.status).toBe(401);
    expect(browserFetch).not.toHaveBeenCalled();
    expect(getLlmFetchTransportPreference('https://api.openai.com')).toBe(
      'native',
    );
  });

  it('clones Request input before native attempt so fallback can reuse body', async () => {
    const browserFetch = vi.fn().mockResolvedValue(new Response('browser-ok'));
    vi.stubGlobal('fetch', browserFetch);
    vi.mocked(tauriPluginFetch).mockRejectedValue(
      new Error('error sending request for url: timed out'),
    );

    const llmFetch = createLlmFetch({ isTauri: true });
    const request = new Request('http://175.115.246.118:4444/v1/chat', {
      method: 'POST',
      body: JSON.stringify({ model: 'test' }),
      headers: { 'Content-Type': 'application/json' },
    });

    const response = await llmFetch(request);
    expect(await response.text()).toBe('browser-ok');
    expect(tauriPluginFetch).toHaveBeenCalledWith(
      expect.any(Request),
      undefined,
    );
    expect(browserFetch).toHaveBeenCalledWith(request, undefined);
  });
});
