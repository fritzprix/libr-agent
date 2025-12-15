/** Represents a single to-do item in the planning state. @internal */
export interface SimpleTodo {
  id: number;
  name: string;
  status: 'pending' | 'completed' | 'blocked';
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  dependsOn?: number[];
}

export interface ScratchpadItem {
  id: number;
  content: string;
  source?: string;
}

/** Represents a single thought in the sequential-thinking tool. @internal */
export interface ThoughtData {
  thought: string;
  thoughtNumber: number;
  totalThoughts: number;
  isRevision?: boolean;
  revisesThought?: number;
  branchFromThought?: number;
  branchId?: string;
  needsMoreThoughts?: boolean;
  nextThoughtNeeded: boolean;
  category?: string;
  relatedTodoId?: number;
  nextAction?: string;
}

/** Represents a single reflection entry. @internal */
export interface ReflectionData {
  critique: string;
  reflection: string;
  nextAction: string;
}

/**
 * Input parameters for the pauseAndThink tool.
 * @internal
 */
export interface PauseAndThinkInput {
  thought: string;
  nextAction?: string;
}

/**
 * Output structure for pauseAndThink tool results.
 * @internal
 */
export interface PauseAndThinkOutput {
  thoughtNumber: number;
  thoughtPreview: string;
  previousThought?: string;
}

/**
 * Represents the entire state of the planning server.
 */
export interface PlanningState {
  /** The current main goal. */
  goal: string | null;
  /** The most recently cleared goal, for context. */
  lastClearedGoal: string | null;
  /** The list of to-do items. */
  todos: SimpleTodo[];
  /** A list of recent notes or temporary records. */
  scratchpad: ScratchpadItem[];
}

/**
 * The base output structure for tool calls, indicating success.
 * @internal
 */
export interface BaseOutput {
  success: boolean;
}

/**
 * The output for the `create_goal` tool call.
 * @internal
 */
export interface CreateGoalOutput extends BaseOutput {
  goal: string;
}

/**
 * The output for the `clear_goal` tool call.
 * @internal
 */
export type ClearGoalOutput = BaseOutput;

/**
 * The output for the `add_todo` tool call.
 * @internal
 */
export interface AddToDoOutput extends BaseOutput {
  todos: SimpleTodo[];
}

/**
 * The output for the `check_todo` tool call.
 * @internal
 */
export interface CheckTodoOutput extends BaseOutput {
  todo: SimpleTodo | null;
  todos: SimpleTodo[];
}
