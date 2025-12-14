import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type { ThoughtData, ReflectionData } from '../types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ThinkingManager');

/**
 * Manages ephemeral sequential thinking state including thoughts, reflections, and branches.
 * This state is kept in-memory and is not persisted to the database.
 *
 * @internal
 */
export class ThinkingManager {
  private thoughtHistory: ThoughtData[] = [];
  private reflectionHistory: ReflectionData[] = [];
  private branches: Record<string, ThoughtData[]> = {};
  private disableThoughtLogging = false;

  constructor() {}

  /**
   * Processes and stores a sequential thought entry.
   * Validates the thought data structure and maintains thought history and branches.
   *
   * @param input - Raw thought data from the tool call
   * @returns MCPResult with processing status and summary
   */
  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    try {
      const data = input as Record<string, unknown>;

      if (!data.thought || typeof data.thought !== 'string') {
        return createMCPErrorToolResult(
          'Invalid thought: must be a string',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (
        data.thoughtNumber === undefined ||
        typeof data.thoughtNumber !== 'number'
      ) {
        return createMCPErrorToolResult(
          'Invalid thoughtNumber: must be a number',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (
        data.totalThoughts === undefined ||
        typeof data.totalThoughts !== 'number'
      ) {
        return createMCPErrorToolResult(
          'Invalid totalThoughts: must be a number',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (typeof data.nextThoughtNeeded !== 'boolean') {
        return createMCPErrorToolResult(
          'Invalid nextThoughtNeeded: must be a boolean',
        ) as MCPResult<Record<string, unknown>>;
      }

      const thought: ThoughtData = {
        thought: data.thought as string,
        thoughtNumber: data.thoughtNumber as number,
        totalThoughts: data.totalThoughts as number,
        nextThoughtNeeded: data.nextThoughtNeeded as boolean,
        isRevision: data.isRevision as boolean | undefined,
        revisesThought: data.revisesThought as number | undefined,
        branchFromThought: data.branchFromThought as number | undefined,
        branchId: data.branchId as string | undefined,
        needsMoreThoughts: data.needsMoreThoughts as boolean | undefined,
        category: data.category as string | undefined,
        relatedTodoId: data.relatedTodoId as number | undefined,
        nextAction: data.nextAction as string | undefined,
      };

      if (thought.thoughtNumber > thought.totalThoughts) {
        thought.totalThoughts = thought.thoughtNumber;
      }

      this.thoughtHistory.push(thought);

      if (thought.branchFromThought && thought.branchId) {
        if (!this.branches[thought.branchId]) {
          this.branches[thought.branchId] = [];
        }
        this.branches[thought.branchId].push(thought);
      }

      if (!this.disableThoughtLogging) {
        logger.info(
          `SEQUENTIAL THOUGHT ${thought.thoughtNumber}/${thought.totalThoughts}: ${thought.thought}`,
        );
      }

      const summary = {
        thoughtNumber: thought.thoughtNumber,
        totalThoughts: thought.totalThoughts,
        nextThoughtNeeded: thought.nextThoughtNeeded,
        branches: Object.keys(this.branches),
        thoughtHistoryLength: this.thoughtHistory.length,
      } as Record<string, unknown>;

      return createMCPStructuredToolResult('Thought processed', summary);
    } catch (error) {
      return createMCPStructuredToolResult('Failed to process thought', {
        error: error instanceof Error ? error.message : String(error),
        status: 'failed',
      });
    }
  }

  /**
   * Processes and stores a critique and reflection entry.
   * Used for agents to reflect on their progress and plan next actions.
   *
   * @param input - Raw reflection data from the tool call
   * @returns MCPResult with formatted reflection and next action guidance
   */
  processCritiqueAndReflection(
    input: unknown,
  ): MCPResult<Record<string, unknown>> {
    try {
      const data = input as Record<string, unknown>;

      if (typeof data.critique !== 'string') {
        return createMCPErrorToolResult(
          'Invalid critique: must be a string',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (typeof data.reflection !== 'string') {
        return createMCPErrorToolResult(
          'Invalid reflection: must be a string',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (typeof data.nextAction !== 'string') {
        return createMCPErrorToolResult(
          'Invalid nextAction: must be a string',
        ) as MCPResult<Record<string, unknown>>;
      }

      const reflectionEntry: ReflectionData = {
        critique: data.critique,
        reflection: data.reflection,
        nextAction: data.nextAction,
      };

      this.reflectionHistory.push(reflectionEntry);

      logger.info(
        `CRITIQUE & REFLECTION: ${reflectionEntry.critique} | ${reflectionEntry.reflection} -> ${reflectionEntry.nextAction}`,
      );

      const message =
        `## Reflection & Critique Analysis\n\n` +
        `**Critique:**\n${reflectionEntry.critique}\n\n` +
        `**Reflection:**\n${reflectionEntry.reflection}\n\n` +
        `**Next Action:**\n${reflectionEntry.nextAction}\n\n` +
        `> Based on this reflection, please proceed with the "Next Action" carefully. Do not repeat this reflection unless new information surfaces.`;

      return createMCPStructuredToolResult(message, {
        success: true,
        reflectionEntry,
      });
    } catch (error) {
      return createMCPStructuredToolResult(
        'Failed to process critique and reflection',
        {
          error: error instanceof Error ? error.message : String(error),
          status: 'failed',
        },
      );
    }
  }

  /**
   * Resets all thinking state (thoughts, reflections, branches).
   * Called when clearing the entire session state.
   */
  reset(): void {
    this.thoughtHistory = [];
    this.reflectionHistory = [];
    this.branches = {};
  }

  /**
   * Gets read-only access to the thought history.
   * @returns Readonly array of thought data
   */
  getThoughtHistory(): readonly ThoughtData[] {
    return this.thoughtHistory;
  }

  /**
   * Gets read-only access to the reflection history.
   * @returns Readonly array of reflection data
   */
  getReflectionHistory(): readonly ReflectionData[] {
    return this.reflectionHistory;
  }

  /**
   * Gets read-only access to the branch mapping.
   * @returns Readonly record of branch IDs to thought arrays
   */
  getBranches(): Readonly<Record<string, ThoughtData[]>> {
    return this.branches;
  }
}
