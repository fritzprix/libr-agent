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
  const completedTodos = state.todos.filter((t) => t.checked).length;
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

  const parsedSubtasks = Array.isArray(subtasks)
    ? subtasks
        .map((item) => parseTodo(item))
        .filter((item): item is SimpleTodo => item !== null)
    : undefined;

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

  const todos = Array.isArray(value.todos)
    ? value.todos
        .map((todo) => parseTodo(todo))
        .filter((todo): todo is SimpleTodo => todo !== null)
    : [];

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

  const items = Array.isArray(value.items)
    ? value.items
        .map((item) => parseScratchpadNote(item))
        .filter((item): item is ScratchpadNote => item !== null)
    : [];

  const count = typeof value.count === 'number' ? value.count : items.length;

  return {
    items,
    count,
  };
}
