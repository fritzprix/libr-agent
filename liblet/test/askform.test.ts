import { describe, expect, it } from 'vitest';

import {
  approvalKeyAction,
  askFormPayload,
  askFormReduce,
  initialAskForm,
} from '../src/askform.js';
import type { UserQuestion } from '../src/types.js';

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

const enter = { input: '', returnKey: true };
const backspace = { input: '', backspaceKey: true };

describe('askFormReduce — single select', () => {
  const questions = [question('a', { options: ['x', 'y'] }), question('b')];

  it('a digit answers the question and advances', () => {
    const state = askFormReduce(initialAskForm(), questions, { input: '2' });
    expect(state.answers.a).toBe('y');
    expect(state.questionIndex).toBe(1);
    expect(state.finished).toBe(false);
  });

  it('out-of-range digits and non-digits are no-ops', () => {
    const before = initialAskForm();
    expect(askFormReduce(before, questions, { input: '9' })).toBe(before);
    expect(askFormReduce(before, questions, { input: '0' })).toBe(before);
    expect(askFormReduce(before, questions, { input: 'z' })).toBe(before);
  });

  it('typing is ignored unless the question allows a custom answer', () => {
    const before = initialAskForm();
    expect(askFormReduce(before, questions, { input: 'o' })).toBe(before);
  });
});

describe('askFormReduce — multi select', () => {
  const questions = [
    question('m', { options: ['a', 'b', 'c'], multiSelect: true }),
  ];

  it('digits accumulate picks and toggling removes one', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: '1' });
    state = askFormReduce(state, questions, { input: '3' });
    expect(state.answers.m).toBe('a,c');
    expect(state.questionIndex).toBe(0);
    state = askFormReduce(state, questions, { input: '3' });
    expect(state.answers.m).toBe('a');
  });

  it('enter advances and the payload is a string array', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: '1' });
    state = askFormReduce(state, questions, { input: '2' });
    state = askFormReduce(state, questions, enter);
    expect(state.finished).toBe(true);
    expect(askFormPayload(state, questions)).toEqual({ m: ['a', 'b'] });
  });

  it('enter with nothing selected is a no-op', () => {
    const before = initialAskForm();
    expect(askFormReduce(before, questions, enter)).toBe(before);
  });
});

describe('askFormReduce — custom answer', () => {
  const questions = [question('c', { options: ['x'], allowCustom: true })];

  it('o starts an explicit mode where digits and o are ordinary text', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: 'o' });
    expect(state.customMode).toBe(true);
    for (const ch of 'room 101')
      state = askFormReduce(state, questions, { input: ch });
    expect(state.pending).toBe('room 101');

    state = askFormReduce(state, questions, backspace);
    expect(state.pending).toBe('room 10');

    state = askFormReduce(state, questions, enter);
    expect(state.finished).toBe(true);
    expect(askFormPayload(state, questions)).toEqual({ c: 'room 10' });
  });

  it('enter with an empty custom answer leaves the mode without advancing', () => {
    const state = askFormReduce(initialAskForm(), questions, { input: 'o' });
    const after = askFormReduce(state, questions, enter);
    expect(after.customMode).toBe(false);
    expect(after.finished).toBe(false);
  });
});

describe('askFormReduce — free text', () => {
  const questions = [question('f')];

  it('every printable character accumulates, digits included', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: '4' });
    state = askFormReduce(state, questions, { input: '2' });
    expect(state.pending).toBe('42');
    state = askFormReduce(state, questions, enter);
    expect(state.finished).toBe(true);
    expect(askFormPayload(state, questions)).toEqual({ f: '42' });
  });

  it('enter on an empty answer is a no-op', () => {
    const before = initialAskForm();
    expect(askFormReduce(before, questions, enter)).toBe(before);
  });
});

describe('askFormReduce — multi-question flow', () => {
  const questions = [
    question('m', { options: ['a', 'b'], multiSelect: true }),
    question('f'),
  ];

  it('finishes only after the last question; payload has finished answers only', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: '1' });
    expect(state.finished).toBe(false);
    state = askFormReduce(state, questions, enter); // multi with a pick
    expect(state.questionIndex).toBe(1);
    state = askFormReduce(state, questions, { input: 'hi' });
    state = askFormReduce(state, questions, enter);
    expect(state.finished).toBe(true);
    const payload = askFormPayload(state, questions);
    expect(payload).toEqual({ m: ['a'], f: 'hi' });
    expect(Object.keys(payload)).toEqual(['m', 'f']); // no scratch keys
  });

  it('a finished form ignores further keys', () => {
    let state = askFormReduce(initialAskForm(), questions, { input: '1' });
    state = askFormReduce(state, questions, enter);
    state = askFormReduce(state, questions, { input: 'hi' });
    state = askFormReduce(state, questions, enter);
    expect(askFormReduce(state, questions, { input: '2' })).toBe(state);
  });
});

describe('approvalKeyAction', () => {
  it('maps the documented keys', () => {
    expect(approvalKeyAction('a', {})).toBe('allow');
    expect(approvalKeyAction('y', {})).toBe('allow');
    expect(approvalKeyAction('d', {})).toBe('deny');
    expect(approvalKeyAction('n', {})).toBe('deny');
    expect(approvalKeyAction('m', {})).toBe('mode');
    expect(approvalKeyAction('', { upArrow: true })).toBe('up');
    expect(approvalKeyAction('', { downArrow: true })).toBe('down');
    expect(approvalKeyAction('x', {})).toBe('none');
  });
});
