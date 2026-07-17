function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export interface ScratchpadNote {
  id: number;
  title: string | null;
  content: string;
}

export interface ScratchpadState {
  items: ScratchpadNote[];
  count: number;
}

function parseScratchpadNote(value: unknown): ScratchpadNote | null {
  if (!isRecord(value)) {
    return null;
  }

  const { id, title, content } = value;

  if (typeof id !== 'number' || typeof content !== 'string') {
    return null;
  }

  return {
    id,
    title: typeof title === 'string' ? title : null,
    content,
  };
}

export function parseScratchpadState(
  value: unknown,
): ScratchpadState | undefined {
  if (!isRecord(value)) {
    return undefined;
  }

  // ⚡ Bolt: Replace .map().filter() with a single-pass loop to avoid intermediate array allocations.
  const items: ScratchpadNote[] = [];
  if (Array.isArray(value.items)) {
    for (let i = 0; i < value.items.length; i++) {
      const parsed = parseScratchpadNote(value.items[i]);
      if (parsed !== null) {
        items.push(parsed);
      }
    }
  }

  const count = typeof value.count === 'number' ? value.count : items.length;

  return {
    items,
    count,
  };
}
