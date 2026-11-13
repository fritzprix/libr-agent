/**
 * Minimal Session API client (docs/api/http_api.md) for the TUI.
 * Bearer auth is sent only when the environment carries the injected key;
 * a keyless loopback backend behaves as before (#2348).
 */

import type {
  AssistantInfo,
  CreateSessionBody,
  GetMessagesResponse,
  HealthStatus,
  ListSessionsResponse,
  PermissionBody,
  PromptBody,
  SessionEvent,
  SessionMetadata,
  SessionStatusResponse,
} from './types.js';

export class ApiError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

export class SessionApiClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(
    port: number,
    private readonly apiKey?: string,
    fetchImpl: typeof fetch = fetch,
  ) {
    this.baseUrl = `http://127.0.0.1:${port}`;
    this.fetchImpl = fetchImpl;
  }

  private async call<T>(
    method: string,
    urlPath: string,
    body?: unknown,
  ): Promise<T> {
    const headers: Record<string, string> = { accept: 'application/json' };
    if (body !== undefined) headers['content-type'] = 'application/json';
    if (this.apiKey) headers.authorization = `Bearer ${this.apiKey}`;
    const res = await this.fetchImpl(`${this.baseUrl}${urlPath}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    if (!res.ok) {
      let message = `HTTP ${res.status}`;
      try {
        const parsed = JSON.parse(text) as { error?: string };
        if (parsed.error) message = parsed.error;
      } catch {
        /* non-JSON body: keep the status text */
      }
      throw new ApiError(res.status, message);
    }
    return (text === '' ? null : JSON.parse(text)) as T;
  }

  health(): Promise<HealthStatus> {
    return this.call<HealthStatus>('GET', '/api/health');
  }

  listSessions(limit = 50): Promise<ListSessionsResponse> {
    return this.call<ListSessionsResponse>(
      'GET',
      `/api/sessions?limit=${limit}`,
    );
  }

  assistants(): Promise<AssistantInfo[]> {
    return this.call<{ assistants: AssistantInfo[] }>(
      'GET',
      '/api/assistants',
    ).then((r) => r.assistants ?? []);
  }

  createSession(body: CreateSessionBody): Promise<SessionStatusResponse> {
    return this.call<unknown>('POST', '/api/sessions', body).then(
      normalizeSession,
    );
  }

  getSession(id: string): Promise<SessionStatusResponse> {
    return this.call<unknown>('GET', `/api/sessions/${id}`).then(
      normalizeSession,
    );
  }

  async messages(id: string, limit = 50): Promise<GetMessagesResponse> {
    const res = await this.call<unknown>(
      'GET',
      `/api/sessions/${id}/messages?limit=${limit}`,
    );
    const messages = Array.isArray(res)
      ? res
      : ((res as GetMessagesResponse)?.messages ?? []);
    return { messages };
  }

  send(id: string, content: string): Promise<{ id: string; status: string }> {
    return this.call('POST', `/api/sessions/${id}/messages`, { content });
  }

  resume(id: string): Promise<unknown> {
    return this.call('POST', `/api/sessions/${id}/resume`);
  }

  terminate(id: string): Promise<unknown> {
    return this.call('POST', `/api/sessions/${id}/terminate`);
  }

  respondPermission(id: string, body: PermissionBody): Promise<unknown> {
    return this.call('POST', `/api/sessions/${id}/channel/permission`, body);
  }

  respondPrompt(id: string, body: PromptBody): Promise<unknown> {
    return this.call('POST', `/api/sessions/${id}/channel/prompt`, body);
  }

  setExecutionMode(
    id: string,
    executionMode: string,
  ): Promise<{
    id: string;
    executionMode: string;
    resolvedApprovalIds: string[];
  }> {
    return this.call('PATCH', `/api/sessions/${id}`, { executionMode });
  }

  /**
   * Follow `GET /api/sessions/:id/events` as an async iterable of SSE
   * events (`status`, `message`, `end`). Ends when the stream ends.
   */
  async *events(id: string): AsyncGenerator<SessionEvent> {
    const headers: Record<string, string> = { accept: 'text/event-stream' };
    if (this.apiKey) headers.authorization = `Bearer ${this.apiKey}`;
    const res = await this.fetchImpl(
      `${this.baseUrl}/api/sessions/${id}/events`,
      { headers },
    );
    if (!res.ok || !res.body) {
      throw new ApiError(res.status, `event stream failed: HTTP ${res.status}`);
    }
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    while (true) {
      const { done, value } = await reader.read();
      if (done) return;
      buffer += decoder.decode(value, { stream: true });
      let sep: number;
      while ((sep = buffer.indexOf('\n\n')) >= 0) {
        const block = buffer.slice(0, sep);
        buffer = buffer.slice(sep + 2);
        const event = parseSseBlock(block);
        if (event) yield event;
      }
    }
  }
}

/** One `event:`/`data:` SSE block; comment/keep-alive blocks yield null. */
export function parseSseBlock(block: string): SessionEvent | null {
  let name = 'message';
  const dataLines: string[] = [];
  for (const line of block.split(/\r?\n/)) {
    if (line.startsWith(':')) continue;
    if (line.startsWith('event:')) name = line.slice(6).trim();
    else if (line.startsWith('data:'))
      dataLines.push(line.slice(5).trimStart());
  }
  if (dataLines.length === 0) return null;
  let data: unknown = dataLines.join('\n');
  try {
    data = JSON.parse(data as string);
  } catch {
    /* keep the raw string */
  }
  return { event: name, data };
}

export type { SessionMetadata };

/** Unwrap `{ session: ... }`-style responses down to the session object. */
export function normalizeSession(raw: unknown): SessionStatusResponse {
  const obj = (raw ?? {}) as Record<string, unknown>;
  const session = (
    obj.session && typeof obj.session === 'object' ? obj.session : obj
  ) as SessionStatusResponse;
  if (!session || typeof session.id !== 'string') {
    throw new ApiError(0, 'backend returned no session object');
  }
  session.pendingApprovals = session.pendingApprovals ?? [];
  return session;
}
