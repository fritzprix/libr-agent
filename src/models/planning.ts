/**
 * Planning types for Agent V2
 * These types are used by both Rust backend planning server and frontend UI
 */

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** Represents a single to-do item in the planning state */
export interface SimpleTodo {
  id: number;
  title: string;
  description?: string;
  checked: boolean;
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  parentId?: number;
  subtasks?: SimpleTodo[];
}

/** Represents a todo with computed blocked/available status */
export interface TodoWithComputedState extends SimpleTodo {
  isBlocked: boolean;
  canStart: boolean;
}

/**
 * Complete planning state for a session.
 * This is returned as structured_state in ServiceContext from the planning server.
 */
export interface PlanningState {
  goal: string | null;
  todos: SimpleTodo[];
  lastUpdated?: string;
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

/**
 * Planning state metadata for display in UI
 */
export interface PlanningMetadata {
  totalTodos: number;
  completedTodos: number;
  activeTodos: number;
}

/**
 * Helper function to calculate planning metadata from state
 */
export function calculatePlanningMetadata(
  state: PlanningState | undefined,
): PlanningMetadata {
  if (!state) {
    return {
      totalTodos: 0,
      completedTodos: 0,
      activeTodos: 0,
    };
  }

  const totalTodos = state.todos.length;

  // ⚡ Bolt: Replaced .reduce() with a single-pass loop to reduce per-element callback overhead.
  let completedTodos = 0;
  for (let i = 0; i < state.todos.length; i++) {
    if (state.todos[i].checked) {
      completedTodos++;
    }
  }

  const activeTodos = totalTodos - completedTodos;

  return {
    totalTodos,
    completedTodos,
    activeTodos,
  };
}

function parseTodo(value: unknown): SimpleTodo | null {
  if (!isRecord(value)) {
    return null;
  }

  const {
    id,
    title,
    description,
    checked,
    summary,
    priority,
    parentId,
    subtasks,
  } = value;

  if (
    typeof id !== 'number' ||
    typeof title !== 'string' ||
    typeof checked !== 'boolean'
  ) {
    return null;
  }

  let parsedSubtasks: SimpleTodo[] | undefined;
  if (Array.isArray(subtasks)) {
    // ⚡ Bolt: Replace .map().filter() with a single-pass loop to avoid intermediate array allocations.
    parsedSubtasks = [];
    for (let i = 0; i < subtasks.length; i++) {
      const parsed = parseTodo(subtasks[i]);
      if (parsed !== null) {
        parsedSubtasks.push(parsed);
      }
    }
  }

  return {
    id,
    title,
    checked,
    description: typeof description === 'string' ? description : undefined,
    summary: typeof summary === 'string' ? summary : undefined,
    priority:
      priority === 'low' || priority === 'medium' || priority === 'high'
        ? priority
        : undefined,
    parentId: typeof parentId === 'number' ? parentId : undefined,
    subtasks: parsedSubtasks,
  };
}

export function parsePlanningState(value: unknown): PlanningState | undefined {
  if (!isRecord(value)) {
    return undefined;
  }

  const goal =
    typeof value.goal === 'string'
      ? value.goal
      : value.goal === null
        ? null
        : null;

  // ⚡ Bolt: Replace .map().filter() with a single-pass loop to avoid intermediate array allocations.
  const todos: SimpleTodo[] = [];
  if (Array.isArray(value.todos)) {
    for (let i = 0; i < value.todos.length; i++) {
      const parsed = parseTodo(value.todos[i]);
      if (parsed !== null) {
        todos.push(parsed);
      }
    }
  }

  const lastUpdated =
    typeof value.lastUpdated === 'string' ? value.lastUpdated : undefined;

  return {
    goal,
    todos,
    lastUpdated,
  };
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
