/**
 * Incremental parser for Ollama-style `<think>...</think>` tags in streamed
 * content deltas. Keeps open/closed mode across chunks so an interrupted
 * stream (no closing tag) never leaks thinking into assistant `text`.
 */

export type ThinkTagStreamMode = 'content' | 'thinking';

export interface ThinkTagStreamState {
  mode: ThinkTagStreamMode;
  /** Incomplete tag prefix held across chunks (e.g. `"<thi"` or `"</thi"`). */
  hold: string;
}

export interface ThinkTagFeedResult {
  content: string;
  thinking: string;
}

const OPEN_TAG_PREFIX = '<think';
const CLOSE_TAG_PREFIX = '</think';

export function createThinkTagStreamState(): ThinkTagStreamState {
  return { mode: 'content', hold: '' };
}

/**
 * Feed one content delta. Emit only bytes that are definitively content or
 * thinking; keep ambiguous tag prefixes in `state.hold`.
 */
export function feedThinkTagDelta(
  state: ThinkTagStreamState,
  delta: string,
): ThinkTagFeedResult {
  let input = `${state.hold}${delta}`;
  state.hold = '';

  let content = '';
  let thinking = '';

  while (input.length > 0) {
    if (state.mode === 'content') {
      const open = findOpenThinkTag(input);
      if (open === null) {
        const holdLen = ambiguousSuffixLength(input, OPEN_TAG_PREFIX);
        if (holdLen > 0) {
          content += input.slice(0, input.length - holdLen);
          state.hold = input.slice(input.length - holdLen);
        } else {
          content += input;
        }
        break;
      }

      if (open.incomplete) {
        content += input.slice(0, open.start);
        state.hold = input.slice(open.start);
        break;
      }

      content += input.slice(0, open.start);
      state.mode = 'thinking';
      input = input.slice(open.end);
      continue;
    }

    const close = findCloseThinkTag(input);
    if (close === null) {
      const holdLen = ambiguousSuffixLength(input, CLOSE_TAG_PREFIX);
      if (holdLen > 0) {
        thinking += input.slice(0, input.length - holdLen);
        state.hold = input.slice(input.length - holdLen);
      } else {
        thinking += input;
      }
      break;
    }

    if (close.incomplete) {
      thinking += input.slice(0, close.start);
      state.hold = input.slice(close.start);
      break;
    }

    thinking += input.slice(0, close.start);
    state.mode = 'content';
    input = input.slice(close.end);
  }

  return { content, thinking };
}

/**
 * End-of-stream flush. Incomplete open tags become thinking mode (no leak into
 * text). Any remaining hold is emitted according to the current mode.
 */
export function flushThinkTagStream(
  state: ThinkTagStreamState,
): ThinkTagFeedResult {
  if (state.hold.length === 0) {
    return { content: '', thinking: '' };
  }

  const hold = state.hold;
  state.hold = '';

  if (state.mode === 'content') {
    // Incomplete `<think...` (no `>` yet): enter thinking, drop the tag text.
    if (startsWithIgnoreCase(hold, OPEN_TAG_PREFIX)) {
      state.mode = 'thinking';
      return { content: '', thinking: '' };
    }
    return { content: hold, thinking: '' };
  }

  // Incomplete `</think...` inside thinking: keep bytes as thinking text.
  return { content: '', thinking: hold };
}

type TagMatch =
  | { start: number; end: number; incomplete: false }
  | { start: number; incomplete: true };

/**
 * Find the next real `<think...>` open tag (not `<thinking`).
 * Incomplete tags at end-of-input are reported so the caller can hold them.
 */
function findOpenThinkTag(text: string): TagMatch | null {
  const lower = text.toLowerCase();
  let from = 0;

  while (from < text.length) {
    const start = lower.indexOf(OPEN_TAG_PREFIX, from);
    if (start === -1) {
      return null;
    }

    const afterPrefix = start + OPEN_TAG_PREFIX.length;
    if (afterPrefix >= text.length) {
      return { start, incomplete: true };
    }

    const boundary = text[afterPrefix];
    // `<thinking` / `<thinkable` — skip; keep searching.
    if (
      boundary !== undefined &&
      boundary !== '>' &&
      !/\s/.test(boundary) &&
      /[a-z0-9_-]/i.test(boundary)
    ) {
      from = start + 1;
      continue;
    }

    const gt = text.indexOf('>', afterPrefix);
    if (gt === -1) {
      return { start, incomplete: true };
    }

    return { start, end: gt + 1, incomplete: false };
  }

  return null;
}

function findCloseThinkTag(text: string): TagMatch | null {
  const lower = text.toLowerCase();
  let from = 0;

  while (from < text.length) {
    const start = lower.indexOf(CLOSE_TAG_PREFIX, from);
    if (start === -1) {
      return null;
    }

    let i = start + CLOSE_TAG_PREFIX.length;
    while (i < text.length && /\s/.test(text[i] ?? '')) {
      i += 1;
    }

    if (i >= text.length) {
      return { start, incomplete: true };
    }

    if (text[i] !== '>') {
      // e.g. `</thinking>` — not our close tag.
      from = start + 1;
      continue;
    }

    return { start, end: i + 1, incomplete: false };
  }

  return null;
}

function startsWithIgnoreCase(value: string, prefix: string): boolean {
  return value.toLowerCase().startsWith(prefix.toLowerCase());
}

/** Length of a suffix that could still grow into `target` (case-insensitive). */
function ambiguousSuffixLength(text: string, target: string): number {
  const lowerText = text.toLowerCase();
  const lowerTarget = target.toLowerCase();
  const max = Math.min(lowerText.length, lowerTarget.length - 1);
  for (let len = max; len >= 1; len -= 1) {
    if (lowerTarget.startsWith(lowerText.slice(lowerText.length - len))) {
      return len;
    }
  }
  return 0;
}
