/**
 * Ink smoke tests (gatekeeper G1.2): drive the real App component through
 * its real useInput handler with a stub client, and assert what the TUI
 * sends back to the backend.
 */

import { describe, expect, it } from 'vitest';
import { render } from 'ink-testing-library';

import { App } from '../src/app.js';
import type { SessionApiClient } from '../src/client.js';
import type {
  PermissionBody,
  PromptBody,
  SessionStatusResponse,
  UserQuestion,
} from '../src/types.js';

function flush(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

interface Calls {
  prompts: PromptBody[];
  permissions: PermissionBody[];
}

function stubClient(calls: Calls): SessionApiClient {
  return {
    events: async function* () {},
    respondPrompt: async (_id: string, body: PromptBody) => {
      calls.prompts.push(body);
    },
    respondPermission: async (_id: string, body: PermissionBody) => {
      calls.permissions.push(body);
    },
    send: async () => ({ id: 'm1', status: 'queued' }),
  } as unknown as SessionApiClient;
}

const status = (
  over: Partial<SessionStatusResponse> = {},
): SessionStatusResponse => ({
  id: 's1',
  status: 'idle',
  model: 'm',
  provider: 'p',
  createdAt: 0,
  updatedAt: 0,
  executionMode: 'normal',
  pendingApprovals: [],
  ...over,
});

const askPrompt = (questions: UserQuestion[]) => ({
  requestId: 'p1',
  kind: 'questions',
  payload: { questions },
  createdAt: '2026-11-13T00:00:00Z',
});

const question = (
  id: string,
  over: Partial<UserQuestion> = {},
): UserQuestion => ({
  id,
  question: `Question ${id}`,
  multiSelect: false,
  allowCustom: false,
  ...over,
});

const renderApp = (initialStatus: SessionStatusResponse) => {
  const calls: Calls = { prompts: [], permissions: [] };
  const view = render(
    <App
      client={stubClient(calls)}
      sessionId="s1"
      initialStatus={initialStatus}
      history={[]}
    />,
  );
  return { calls, ...view };
};

describe('App askUser form (real useInput path)', () => {
  it('multi-select keeps every pick, renders checkmarks, and submits an array', async () => {
    const questions = [
      question('q1', {
        options: ['a', 'b', 'c'],
        multiSelect: true,
        allowCustom: true,
      }),
    ];
    const { calls, stdin, lastFrame } = renderApp(
      status({ pendingUserPrompt: askPrompt(questions) }),
    );
    await flush();

    stdin.write('1');
    stdin.write('2');
    await flush();
    expect(lastFrame()).toContain('a ✓');
    expect(lastFrame()).toContain('b ✓');
    expect(calls.prompts).toHaveLength(0);

    stdin.write('\r');
    await flush();
    expect(calls.prompts).toHaveLength(1);
    expect(calls.prompts[0].requestId).toBe('p1');
    expect(JSON.parse(calls.prompts[0].value as string)).toEqual({
      q1: ['a', 'b'],
    });
  });

  it('custom mode keeps the letter o and digits inside the answer', async () => {
    const questions = [question('q1', { options: ['x'], allowCustom: true })];
    const { calls, stdin } = renderApp(
      status({ pendingUserPrompt: askPrompt(questions) }),
    );
    await flush();

    stdin.write('o');
    await flush();
    stdin.write('room 101');
    await flush();
    stdin.write('\r');
    await flush();
    expect(JSON.parse(calls.prompts[0].value as string)).toEqual({
      q1: 'room 101',
    });
  });

  it('esc cancels without sending answers', async () => {
    const questions = [question('q1', { options: ['x'] })];
    const { calls, stdin } = renderApp(
      status({ pendingUserPrompt: askPrompt(questions) }),
    );
    await flush();

    stdin.write('\u001b');
    await flush();
    expect(calls.prompts).toHaveLength(1);
    expect(calls.prompts[0].action).toBe('cancel');
  });
});

describe('App approvals and secrets', () => {
  it('a responds allow to the selected approval', async () => {
    const { calls, stdin } = renderApp(
      status({
        pendingApprovals: [
          {
            toolCallId: 't1',
            toolName: 'shell',
            reason: 'rm',
            commandPreview: 'rm -rf /tmp/x',
            approvalKind: 'standard',
            requestId: 'r1',
          },
        ],
      }),
    );
    await flush();

    stdin.write('a');
    await flush();
    expect(calls.permissions).toEqual([
      { requestId: 'r1', toolCallId: 't1', behavior: 'allow' },
    ]);
  });

  it('secret prompt sends the typed value on enter', async () => {
    const { calls, stdin } = renderApp(
      status({
        pendingUserPrompt: {
          requestId: 'p2',
          kind: 'secret',
          payload: { variableName: 'TOKEN' },
          createdAt: '2026-11-13T00:00:00Z',
        },
      }),
    );
    await flush();

    stdin.write('s3cret');
    await flush();
    stdin.write('\r');
    await flush();
    expect(calls.prompts).toEqual([
      { requestId: 'p2', action: 'answer', value: 's3cret' },
    ]);
  });

  it('attachedRunning shows the driven-by-another-frontend header', async () => {
    const calls: Calls = { prompts: [], permissions: [] };
    const { lastFrame } = render(
      <App
        client={stubClient(calls)}
        sessionId="s1"
        initialStatus={status({ status: 'busy' })}
        history={[]}
        attachedRunning
      />,
    );
    await flush();
    expect(lastFrame()).toContain('driven by another frontend');
  });
});
