/**
 * Pure state logic for the interactive prompts (gatekeeper G1.1/G1.2 on #2741).
 *
 * The askUser form, its answer payload, and the approval key mapping live
 * here — free of React and Ink — so they can be tested directly.
 *
 * Key rules (the three failures G1.1 found):
 * - in-progress "other"/free-text input is kept in `pending`, never inside
 *   `answers`, so the submitted payload contains finished answers only;
 * - a multiSelect question accumulates every picked option;
 * - custom entry is an explicit mode (`o` starts it): while in it, digits
 *   and the letter `o` are ordinary text instead of option picks.
 */

import type { UserQuestion } from './types.js';

export interface AskFormState {
  questionIndex: number;
  /** Answers keyed by question id; a multiSelect answer is the list of picks. */
  answers: Record<string, string | string[]>;
  /** In-progress "other" / free-text input for the current question. */
  pending: string;
  /** True while typing the custom answer (started with `o`). */
  customMode: boolean;
  /** True once the last question was answered; submit `askFormPayload` then. */
  finished: boolean;
}

/** One printable-key press on the form (control keys are decided by the caller). */
export interface AskKey {
  input: string;
  returnKey?: boolean;
  backspaceKey?: boolean;
}

export function initialAskForm(): AskFormState {
  return {
    questionIndex: 0,
    answers: {},
    pending: '',
    customMode: false,
    finished: false,
  };
}

function advance(
  state: AskFormState,
  questions: UserQuestion[],
  answers: Record<string, string | string[]>,
): AskFormState {
  const next = state.questionIndex + 1;
  return {
    questionIndex: next,
    answers,
    pending: '',
    customMode: false,
    finished: next >= questions.length,
  };
}

/**
 * Reduce one key press. Returns the same state object when the key is a
 * no-op, so callers can detect change by identity.
 */
export function askFormReduce(
  state: AskFormState,
  questions: UserQuestion[],
  key: AskKey,
): AskFormState {
  if (state.finished || questions.length === 0) return state;
  const question = questions[state.questionIndex];
  if (!question) return state;
  const options = question.options ?? [];
  const current = state.answers[question.id];
  const picks = Array.isArray(current) ? current : [];

  if (key.returnKey) {
    if (state.pending)
      return advance(state, questions, {
        ...state.answers,
        [question.id]: question.multiSelect
          ? [...picks, state.pending]
          : state.pending,
      });
    if (question.multiSelect && picks.length > 0)
      return advance(state, questions, state.answers);
    if (options.length > 0 && state.customMode)
      return { ...state, customMode: false }; // empty custom entry leaves the mode
    return state;
  }

  if (key.backspaceKey)
    return { ...state, pending: state.pending.slice(0, -1) };

  if (state.customMode) {
    return key.input === ''
      ? state
      : { ...state, pending: state.pending + key.input };
  }

  if (options.length > 0) {
    const value = Number(key.input);
    if (
      key.input !== '' &&
      Number.isInteger(value) &&
      value >= 1 &&
      value <= options.length
    ) {
      const option = options[value - 1];
      if (question.multiSelect) {
        const next = picks.includes(option)
          ? picks.filter((c) => c !== option)
          : [...picks, option];
        return {
          ...state,
          answers: { ...state.answers, [question.id]: next },
        };
      }
      return advance(state, questions, {
        ...state.answers,
        [question.id]: option,
      });
    }
    if (question.allowCustom && key.input === 'o')
      return { ...state, customMode: true };
    return state;
  }

  // Free-text question: every printable character accumulates.
  return key.input === ''
    ? state
    : { ...state, pending: state.pending + key.input };
}

/**
 * The exact JSON value sent as the `answer` prompt response: finished
 * answers only, multiSelect as `string[]`, no scratch keys.
 */
export function askFormPayload(
  state: AskFormState,
  questions: UserQuestion[],
): Record<string, string | string[]> {
  const payload: Record<string, string | string[]> = {};
  for (const question of questions) {
    const raw = state.answers[question.id];
    if (raw === undefined || raw.length === 0) continue;
    payload[question.id] = raw;
  }
  return payload;
}

export type ApprovalKey = 'up' | 'down' | 'allow' | 'deny' | 'mode' | 'none';

/** Map one key press to an approval-list action. */
export function approvalKeyAction(
  input: string,
  key: { upArrow?: boolean; downArrow?: boolean },
): ApprovalKey {
  if (key.upArrow) return 'up';
  if (key.downArrow) return 'down';
  if (input === 'a' || input === 'y') return 'allow';
  if (input === 'd' || input === 'n') return 'deny';
  if (input === 'm') return 'mode';
  return 'none';
}
