import { describe, expect, it } from 'vitest';
import {
  classifyAgentSessionCard,
  parseAgentSessionToolResult,
  resolveAgentSessionId,
} from '../agent-types';
import { canRenderStructuredToolResult } from '../ToolStructuredResult';

describe('agent session structured types', () => {
  it('parseAgentSessionToolResult accepts checkSession harvest payload', () => {
    const parsed = parseAgentSessionToolResult({
      toolName: 'agent__checkSession',
      sessionId: 'a1b2c3d4e5',
      status: 'idle',
      responseStatus: 'success',
      turnCount: 4,
      result: 'Final answer',
      assistantName: 'Writer',
    });
    expect(parsed).not.toBeNull();
    expect(parsed?.result).toBe('Final answer');
    expect(
      classifyAgentSessionCard('agent__checkSession', parsed!),
    ).toBe('finished');
  });

  it('classifies wait timeout separately from in-progress', () => {
    const parsed = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'busy',
      responseStatus: 'timeout',
      timeout: true,
      timeoutSeconds: 3600,
      turnCount: 7,
    });
    expect(parsed).not.toBeNull();
    expect(classifyAgentSessionCard('agent__checkSession', parsed!)).toBe(
      'wait_timeout',
    );
    expect(classifyAgentSessionCard('agent__startSession', parsed!)).toBe(
      'wait_timeout',
    );
  });

  it('classifies spawn and instruction ack as pending outcomes', () => {
    const spawn = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'started',
      responseStatus: 'pending',
    });
    expect(classifyAgentSessionCard('agent__startSession', spawn!)).toBe(
      'spawned',
    );

    const inject = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      messageId: 'msg-1',
      status: 'accepted',
      responseStatus: 'pending',
    });
    expect(classifyAgentSessionCard('agent__messageToSession', inject!)).toBe(
      'instruction_sent',
    );
  });

  it('does not treat responseStatus success alone as harvest finished', () => {
    // Regression: success/result heuristics must not override spawn/inject acks
    // or mis-label non-terminal session status.
    const falseSpawnHarvest = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'started',
      responseStatus: 'success',
      result: 'misleading body',
      task: 'do something',
    });
    expect(
      classifyAgentSessionCard('agent__startSession', falseSpawnHarvest!),
    ).toBe('spawned');

    const falseInjectHarvest = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'busy',
      responseStatus: 'success',
      result: 'not a final answer',
      instruction: 'follow up',
    });
    expect(
      classifyAgentSessionCard('agent__messageToSession', falseInjectHarvest!),
    ).toBe('instruction_sent');
  });

  it('classifies wait-settled startSession/messageToSession as harvest kinds', () => {
    const waitSettled = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'idle',
      responseStatus: 'success',
      result: 'Final answer',
    });
    expect(classifyAgentSessionCard('agent__startSession', waitSettled!)).toBe(
      'finished',
    );
    expect(
      classifyAgentSessionCard('agent__messageToSession', waitSettled!),
    ).toBe('finished');

    const waitFailed = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'error',
      responseStatus: 'error',
      result: 'boom',
    });
    expect(classifyAgentSessionCard('agent__startSession', waitFailed!)).toBe(
      'needs_attention',
    );
  });

  it('classifies paused/error/terminated as needs_attention', () => {
    const paused = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'paused',
      responseStatus: 'paused',
      result: 'Waiting on approval',
    });
    expect(classifyAgentSessionCard('agent__checkSession', paused!)).toBe(
      'needs_attention',
    );

    const failed = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      status: 'error',
      responseStatus: 'error',
      result: 'Permission denied',
    });
    expect(classifyAgentSessionCard('agent__checkSession', failed!)).toBe(
      'needs_attention',
    );
  });

  it('classifies stop and delete', () => {
    const stopped = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      stopped: true,
      status: 'terminated',
      responseStatus: 'success',
    });
    expect(classifyAgentSessionCard('agent__stopSession', stopped!)).toBe(
      'stopped',
    );

    const stopNoop = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      stopped: false,
      status: 'idle',
      responseStatus: 'noop',
    });
    expect(classifyAgentSessionCard('agent__stopSession', stopNoop!)).toBe(
      'stopped',
    );

    const stopFailed = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      stopped: false,
      status: 'busy',
      responseStatus: 'error',
    });
    expect(classifyAgentSessionCard('agent__stopSession', stopFailed!)).toBe(
      'needs_attention',
    );

    const deleted = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      deleted: true,
      descendantCount: 2,
      deletedIds: ['a1b2c3d4e5', 'bbbbbbbbbb', 'cccccccccc'],
      responseStatus: 'success',
    });
    expect(classifyAgentSessionCard('agent__deleteSession', deleted!)).toBe(
      'deleted',
    );

    const deleteWithoutMarker = parseAgentSessionToolResult({
      sessionId: 'a1b2c3d4e5',
      responseStatus: 'success',
    });
    expect(
      classifyAgentSessionCard('agent__deleteSession', deleteWithoutMarker!),
    ).toBeNull();
  });

  it('rejects payloads without session identity', () => {
    expect(parseAgentSessionToolResult({ responseStatus: 'pending' })).toBeNull();
  });

  it('resolveAgentSessionId prefers sessionId over resourceId', () => {
    expect(
      resolveAgentSessionId({
        sessionId: 'abc',
        resourceId: 'xyz',
      }),
    ).toBe('abc');
  });

  it('canRenderStructuredToolResult accepts agent session tools', () => {
    expect(
      canRenderStructuredToolResult('agent__checkSession', {
        sessionId: 'a1b2c3d4e5',
        status: 'idle',
        responseStatus: 'success',
        result: 'done',
      }),
    ).toBe(true);
    expect(
      canRenderStructuredToolResult('agent__startSession', {
        sessionId: 'a1b2c3d4e5',
        status: 'started',
        responseStatus: 'pending',
      }),
    ).toBe(true);
    expect(
      canRenderStructuredToolResult('agent__listAgents', {
        sessionId: 'a1b2c3d4e5',
      }),
    ).toBe(false);
  });
});
