import type { SimpleTodo } from '../types';

export interface CircularDependencyError {
  todoId: number;
  cycle: number[];
}

/**
 * Detects circular dependencies in todo dependency graph using Depth-First Search (DFS)
 *
 * This function validates that adding or updating a todo with the given dependencies
 * will not create a circular dependency in the task graph. A circular dependency occurs
 * when a chain of dependencies forms a cycle (e.g., A depends on B, B depends on C, C depends on A).
 *
 * Algorithm: DFS-based cycle detection with recursion stack tracking
 * - Time complexity: O(V + E) where V is number of todos and E is number of dependencies
 * - Space complexity: O(V) for visited sets and recursion stack
 *
 * @param todos - All existing todos in the current session/thread
 * @param newTodoId - ID of the todo being created/updated
 * @param newDependencies - Proposed dependencies for the todo
 * @returns CircularDependencyError with cycle path if cycle detected, null otherwise
 *
 * @example
 * // Existing: Todo 1, Todo 2 (depends on 1)
 * // Try to make Todo 1 depend on Todo 2 - creates cycle
 * const error = detectCircularDependency(todos, 1, [2]);
 * // Returns: { todoId: 1, cycle: [1, 2, 1] }
 *
 * @example
 * // Valid DAG structure
 * // Todo 1 depends on 3, Todo 2 depends on 3, Todo 4 depends on 1 and 2
 * const error = detectCircularDependency(todos, 4, [1, 2]);
 * // Returns: null (no cycle)
 */
export function detectCircularDependency(
  todos: SimpleTodo[],
  newTodoId: number,
  newDependencies: number[],
): CircularDependencyError | null {
  // Build adjacency list representation of the dependency graph
  // Key: todo ID, Value: array of IDs that this todo depends on
  const graph = new Map<number, number[]>();

  // Add existing todos to graph (excluding the one being created/updated)
  for (const todo of todos) {
    if (todo.id !== newTodoId && todo.dependsOn && todo.dependsOn.length > 0) {
      graph.set(todo.id, todo.dependsOn);
    }
  }

  // Add the new/updated todo with its proposed dependencies
  graph.set(newTodoId, newDependencies);

  // Depth-First Search for cycle detection
  const visited = new Set<number>();
  const recursionStack = new Set<number>(); // Tracks nodes in current DFS path
  const path: number[] = []; // Stores current DFS path for cycle extraction

  /**
   * DFS helper function to detect cycles starting from a given node
   * @param nodeId - Current node being visited
   * @returns Array representing the cycle if found, null otherwise
   */
  function dfs(nodeId: number): number[] | null {
    visited.add(nodeId);
    recursionStack.add(nodeId);
    path.push(nodeId);

    const neighbors = graph.get(nodeId) || [];

    for (const neighbor of neighbors) {
      if (!visited.has(neighbor)) {
        // Explore unvisited neighbor
        const cycle = dfs(neighbor);
        if (cycle) return cycle;
      } else if (recursionStack.has(neighbor)) {
        // Found a back edge - cycle detected!
        // Extract the cycle from the path
        const cycleStart = path.indexOf(neighbor);
        return path.slice(cycleStart).concat(neighbor);
      }
      // If visited but not in recursion stack, it's a cross edge (already explored path)
    }

    // Backtrack: remove from recursion stack and path
    recursionStack.delete(nodeId);
    path.pop();
    return null;
  }

  // Start DFS from the new/updated todo
  // We only need to check from this node since we're validating its new dependencies
  const cycle = dfs(newTodoId);

  if (cycle) {
    return {
      todoId: newTodoId,
      cycle,
    };
  }

  return null;
}
