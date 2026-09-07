import { describe, it, expect } from 'vitest';
import { StreamAccumulator } from '../execute-completion/stream-accumulator';
import type { Settings } from '@/lib/services/settings-service';

function createAccumulator(): StreamAccumulator {
  const settingsRef = {
    current: {
      advanced: {},
    } as Settings,
  };
  return new StreamAccumulator(
    'test-session',
    'msg-1',
    settingsRef,
    performance.now(),
  );
}

describe('StreamAccumulator streamingPhase lifecycle', () => {
  it('initializes with prefill phase', () => {
    const accumulator = createAccumulator();
    expect(accumulator.currentPhase).toBe('prefill');
  });

  it('transitions to thinking phase on thinking chunk', () => {
    const accumulator = createAccumulator();
    accumulator.processChunk({ thinking: 'Analyzing the problem...' });
    expect(accumulator.currentPhase).toBe('thinking');
    expect(accumulator.content).toHaveLength(1);
    expect(accumulator.content[0].type).toBe('thinking');
  });

  it('transitions to generating phase on text content chunk', () => {
    const accumulator = createAccumulator();
    accumulator.processChunk({ content: 'Here is the answer.' });
    expect(accumulator.currentPhase).toBe('generating');
    expect(accumulator.content).toHaveLength(1);
    expect(accumulator.content[0].type).toBe('text');
  });

  it('transitions to tool_calling phase on tool_calls delta chunk', () => {
    const accumulator = createAccumulator();
    accumulator.processChunk({
      tool_calls: [
        {
          index: 0,
          id: 'call-1',
          function: {
            name: 'bash',
            arguments: '{"cmd": "ls"}',
          },
        },
      ],
    });
    expect(accumulator.currentPhase).toBe('tool_calling');
    expect(accumulator.content).toHaveLength(1);
    expect(accumulator.content[0].type).toBe('tool_call');
  });

  it('transitions through thinking -> generating -> tool_calling cleanly', () => {
    const accumulator = createAccumulator();
    expect(accumulator.currentPhase).toBe('prefill');

    // 1. Thinking
    accumulator.processChunk({ thinking: 'Thinking step 1' });
    expect(accumulator.currentPhase).toBe('thinking');

    // 2. Generating text
    accumulator.processChunk({ content: 'Let me run a command.' });
    expect(accumulator.currentPhase).toBe('generating');

    // 3. Tool Calling
    accumulator.processChunk({
      tool_call_starts: [
        {
          index: 0,
          id: 'call-1',
          function: { name: 'bash', arguments: '' },
        },
      ],
    });
    expect(accumulator.currentPhase).toBe('tool_calling');
  });

  it('prioritizes generating over thinking when both appear in the same chunk', () => {
    const accumulator = createAccumulator();
    accumulator.processChunk({
      thinking: 'Analyzing data...',
      content: 'Here is the final answer.',
    });
    expect(accumulator.currentPhase).toBe('generating');
  });

  it('prioritizes tool_calling over content and thinking when tool_calls appear in the chunk', () => {
    const accumulator = createAccumulator();
    accumulator.processChunk({
      thinking: 'Need to run bash',
      content: 'Running now...',
      tool_calls: [
        {
          index: 0,
          id: 'call-2',
          function: { name: 'bash', arguments: '{"cmd":"pwd"}' },
        },
      ],
    });
    expect(accumulator.currentPhase).toBe('tool_calling');
  });
});
