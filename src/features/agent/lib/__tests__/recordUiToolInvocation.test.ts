import { beforeEach, describe, expect, it, vi } from 'vitest';

import { recordUiToolInvocation } from '../recordUiToolInvocation';

const createToolMessagePair = vi.hoisted(() => vi.fn());
const createUserMessage = vi.hoisted(() => vi.fn());
const createId = vi.hoisted(() => vi.fn(() => 'tool-call-id'));

vi.mock('@paralleldrive/cuid2', () => ({
  createId,
}));

vi.mock('@/lib/chat-utils', () => ({
  createToolMessagePair,
  createUserMessage,
}));

describe('recordUiToolInvocation', () => {
  const appendToolMessages = vi.fn().mockResolvedValue(undefined);
  const submit = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    appendToolMessages.mockClear();
    submit.mockClear();
    createToolMessagePair.mockReset();
    createUserMessage.mockReset();
    createId.mockReturnValue('tool-call-id');

    createToolMessagePair.mockReturnValue([
      { id: 'call', role: 'assistant' },
      { id: 'result', role: 'tool' },
    ]);
    createUserMessage.mockReturnValue({ id: 'user', role: 'user' });
  });

  it('appends only for history-only neg-list tools', async () => {
    const result = await recordUiToolInvocation(
      { appendToolMessages, submit },
      {
        toolName: 'workspace__importFiles',
        params: { files: [] },
        result: [{ type: 'text', text: 'ok' }],
        sessionId: 'session-1',
      },
    );

    expect(result.mode).toBe('history-only');
    expect(appendToolMessages).toHaveBeenCalledTimes(1);
    expect(submit).not.toHaveBeenCalled();
  });

  it('appends then submits for playbook selectPlaybook', async () => {
    const result = await recordUiToolInvocation(
      { appendToolMessages, submit },
      {
        toolName: 'playbook__selectPlaybook',
        params: { id: 'pb-1' },
        result: [{ type: 'text', text: 'selected' }],
        sessionId: 'session-1',
        assistantId: 'asst-1',
        kickoffUserText: 'Execute the selected playbook.',
      },
    );

    expect(result.mode).toBe('history-then-submit');
    expect(appendToolMessages).toHaveBeenCalledTimes(1);
    expect(createUserMessage).toHaveBeenCalledWith(
      'Execute the selected playbook.',
      'session-1',
      undefined,
      'asst-1',
      'ui',
    );
    expect(submit).toHaveBeenCalledTimes(1);
  });

  it('rejects history-then-submit without kickoff text', async () => {
    await expect(
      recordUiToolInvocation(
        { appendToolMessages, submit },
        {
          toolName: 'playbook__selectPlaybook',
          params: { id: 'pb-1' },
          result: [{ type: 'text', text: 'selected' }],
          sessionId: 'session-1',
        },
      ),
    ).rejects.toThrow(/kickoffUserText is required/);

    expect(submit).not.toHaveBeenCalled();
  });
});
