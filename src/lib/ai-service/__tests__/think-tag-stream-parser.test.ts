import { describe, expect, it } from 'vitest';
import {
  createThinkTagStreamState,
  feedThinkTagDelta,
  flushThinkTagStream,
} from '../think-tag-stream-parser';

describe('think-tag-stream-parser', () => {
  it('splits a complete think block in one feed', () => {
    const state = createThinkTagStreamState();
    const result = feedThinkTagDelta(
      state,
      '<think>  thinking inside tags  </think>  content with spaces  ',
    );

    expect(result.thinking).toBe('  thinking inside tags  ');
    expect(result.content).toBe('  content with spaces  ');
    expect(state.mode).toBe('content');
  });

  it('classifies unclosed think as thinking, never as content', () => {
    const state = createThinkTagStreamState();
    const result = feedThinkTagDelta(
      state,
      '<think>\nI was still reasoning when maxTokens hit',
    );

    expect(result.content).toBe('');
    expect(result.thinking).toBe(
      '\nI was still reasoning when maxTokens hit',
    );
    expect(state.mode).toBe('thinking');
  });

  it('keeps mode across chunked open/close tags', () => {
    const state = createThinkTagStreamState();

    expect(feedThinkTagDelta(state, '<thi')).toEqual({
      content: '',
      thinking: '',
    });
    expect(state.hold.toLowerCase()).toBe('<thi');

    expect(feedThinkTagDelta(state, 'nk>part1')).toEqual({
      content: '',
      thinking: 'part1',
    });
    expect(state.mode).toBe('thinking');

    expect(feedThinkTagDelta(state, ' part2</thi')).toEqual({
      content: '',
      thinking: ' part2',
    });

    expect(feedThinkTagDelta(state, 'nk> final')).toEqual({
      content: ' final',
      thinking: '',
    });
    expect(state.mode).toBe('content');
  });

  it('does not treat <thinking> as a think open tag', () => {
    const state = createThinkTagStreamState();
    const result = feedThinkTagDelta(
      state,
      '<thinking>not a think tag</thinking>ok',
    );

    expect(result.thinking).toBe('');
    expect(result.content).toBe(
      '<thinking>not a think tag</thinking>ok',
    );
  });

  it('flush drops incomplete open tag into thinking mode without leaking text', () => {
    const state = createThinkTagStreamState();
    const fed = feedThinkTagDelta(state, 'before <think');

    expect(fed.content).toBe('before ');
    expect(fed.thinking).toBe('');
    expect(state.hold.toLowerCase()).toBe('<think');

    const flushed = flushThinkTagStream(state);

    expect(flushed.content).toBe('');
    expect(flushed.thinking).toBe('');
    expect(state.mode).toBe('thinking');
    expect(state.hold).toBe('');
  });

  it('supports multiple think blocks', () => {
    const state = createThinkTagStreamState();
    const result = feedThinkTagDelta(
      state,
      '<think>a</think>mid<think>b</think>end',
    );

    expect(result.thinking).toBe('ab');
    expect(result.content).toBe('midend');
  });
});
