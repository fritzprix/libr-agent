/**
 * Memory types for Agent V2
 * These types correspond to the `memory` builtin MCP server's structured_state.
 */

/** Represents a single memory note */
export interface MemoryItem {
  id: number;
  title?: string;
  content: string;
  tags?: string[];
  source?: string;
}

/**
 * Memory state returned as structured_state in ServiceContext from the memory server.
 */
export interface MemoryState {
  items: MemoryItem[];
  count: number;
}
