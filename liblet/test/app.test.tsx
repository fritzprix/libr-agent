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
  terminated: string[];
}

type Events = SessionApiClient['events'];

function stubClient(
  calls: Calls,
  events: Events = async function* () {},
): SessionApiClient {
  return {
    events,
    terminate: async (id: string) => {
      calls.terminated.push(id);
    },
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

const renderApp = (initialStatus: SessionStatusResponse, events?: Events) => {
  const calls: Calls = { prompts: [], permissions: [], terminated: [] };
  const view = render(
    <App
      client={stubClient(calls, events)}
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

  it('a shrinking approval list keeps a valid selection', async () => {
    const approval = (n: number) => ({
      toolCallId: `t${n}`,
      toolName: 'shell',
      reason: 'run',
      commandPreview: `cmd ${n}`,
      approvalKind: 'standard',
      requestId: `r${n}`,
    });
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    const { calls, stdin, lastFrame } = renderApp(
      status({ pendingApprovals: [approval(1), approval(2)] }),
      async function* () {
        await gate;
        yield {
          event: 'status',
          data: status({ pendingApprovals: [approval(1)] }),
        };
        await new Promise(() => {}); // keep the stream open
      },
    );
    await flush();

    stdin.write('\u001b[B'); // select the second approval
    await flush();
    stdin.write('a');
    await flush();
    release(); // the backend resolves it; one approval is left
    await flush();
    await flush();
    expect(lastFrame()).toContain('approval needed (1/1)');

    stdin.write('a');
    await flush();
    expect(calls.permissions.map((p) => p.requestId)).toEqual(['r2', 'r1']);
  });

  it('ctrl+c interrupts a running turn instead of quitting', async () => {
    const { calls, stdin } = renderApp(status({ status: 'busy' }));
    await flush();

    stdin.write('\u0003');
    await flush();
    expect(calls.terminated).toEqual(['s1']);
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
    const calls: Calls = { prompts: [], permissions: [], terminated: [] };
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
