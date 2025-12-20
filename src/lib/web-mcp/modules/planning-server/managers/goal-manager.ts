import { createMCPStructuredToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { db, type PlanningGoal } from '../db';
import type { CreateGoalOutput, ClearGoalOutput } from '../types';
import { buildEmptyTitleError } from '../utils/response-builders';

/**
 * Manages goal operations including creation, updates, and retrieval.
 * Goals represent high-level objectives for a planning session.
 *
 * @internal
 */
export class GoalManager {
  constructor(
    private sessionId: string,
    private threadId: string,
  ) {}

  /**
   * Retrieves the currently active goal for this session/thread.
   *
   * @returns The active goal record, or undefined if no active goal exists
   */
  private async getActiveGoal(): Promise<PlanningGoal | undefined> {
    return db.goals
      .where({
        sessionId: this.sessionId,
        threadId: this.threadId,
        isActive: 1,
      })
      .last();
  }

  /**
   * Retrieves the most recently cleared (inactive) goal for this session/thread.
   *
   * @returns The last cleared goal record, or undefined if none exists
   */
  private async getLastClearedGoalRecord(): Promise<PlanningGoal | undefined> {
    return db.goals
      .where({
        sessionId: this.sessionId,
        threadId: this.threadId,
        isActive: 0,
      })
      .last();
  }

  /**
   * Creates a new goal for the session. If a previous goal exists, it is deactivated.
   * Provides guidance on next actions after goal creation.
   *
   * @param goal - The goal content/description
   * @param existingTodosCount - Number of existing todos (provided by caller for context)
   * @returns MCPResult with goal creation status and next action suggestions
   */
  async createGoal(
    goal: string,
    existingTodosCount: number,
  ): Promise<MCPResult<CreateGoalOutput>> {
    // Validation: Goal name cannot be empty or whitespace-only
    if (!goal || goal.trim() === '') {
      return buildEmptyTitleError('goal') as MCPResult<CreateGoalOutput>;
    }

    const previousGoal = await this.getActiveGoal();

    // Deactivate previous goal if exists
    if (previousGoal && previousGoal.id) {
      await db.goals.update(previousGoal.id, { isActive: 0 });
    }

    await db.goals.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: goal,
      isActive: 1,
      createdAt: Date.now(),
    });

    const nextActions = [
      'Break down goal into actionable todos with addTodo',
      'Set priorities and dependencies if needed',
      'Track progress with getCurrentState',
    ];

    let message = `Goal set: "${goal}"`;
    if (previousGoal) {
      message += `\n\nPrevious goal: "${previousGoal.content}"\nTodos from previous goal: ${existingTodosCount}`;
    }

    return new MCPResponseBuilder({
      goal,
      success: true,
      previousGoal: previousGoal?.content,
      existingTodos: existingTodosCount,
    })
      .withMessage(message)
      .withNextActions(nextActions)
      .withSuggestions([
        'Start with 3-5 high-level todos, then refine as you go',
      ])
      .asSuccess();
  }

  /**
   * Updates the content of the currently active goal.
   *
   * @param goal - The new goal content
   * @returns MCPResult with update status
   */
  async updateGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    // Validation: Goal name cannot be empty or whitespace-only
    if (!goal || goal.trim() === '') {
      return buildEmptyTitleError('goal') as MCPResult<CreateGoalOutput>;
    }

    const activeGoal = await this.getActiveGoal();
    if (!activeGoal || !activeGoal.id) {
      return createMCPStructuredToolResult(
        'No active goal to update. Use createGoal first.',
        {
          success: false,
          goal: '',
        },
      );
    }
    const oldGoalContent = activeGoal.content;
    await db.goals.update(activeGoal.id, { content: goal });

    return createMCPStructuredToolResult<CreateGoalOutput>(
      `Goal updated from "${oldGoalContent}" to "${goal}"`,
      {
        goal,
        success: true,
      },
    );
  }

  /**
   * Clears (deactivates) the currently active goal.
   *
   * @param remainingTodosCount - Number of remaining todos (provided by caller for context)
   * @returns MCPResult with clear status and remaining todos info
   */
  async clearGoal(
    remainingTodosCount: number,
  ): Promise<MCPResult<ClearGoalOutput>> {
    const activeGoal = await this.getActiveGoal();
    if (activeGoal && activeGoal.id) {
      const clearedGoalContent = activeGoal.content;
      await db.goals.update(activeGoal.id, { isActive: 0 });

      const todoSummary =
        remainingTodosCount > 0
          ? `Remaining todos: ${remainingTodosCount}`
          : 'All todos have been completed or cleared.';
      return createMCPStructuredToolResult<ClearGoalOutput>(
        `Goal cleared: "${clearedGoalContent}"\n${todoSummary}\nSession is now ready for a new goal.`,
        {
          success: true,
        },
      );
    }
    return createMCPStructuredToolResult('No active goal to clear', {
      success: false,
    });
  }

  /**
   * Retrieves the content of the currently active goal.
   *
   * @returns The goal content string, or null if no active goal
   */
  async getGoal(): Promise<string | null> {
    const goal = await this.getActiveGoal();
    return goal ? goal.content : null;
  }

  /**
   * Retrieves the content of the most recently cleared goal.
   *
   * @returns The cleared goal content string, or null if no cleared goal exists
   */
  async getLastClearedGoal(): Promise<string | null> {
    const goal = await this.getLastClearedGoalRecord();
    return goal ? goal.content : null;
  }

  /**
   * Clears all goals for this session/thread (both active and inactive).
   * Used when performing a complete session reset.
   *
   * @returns The number of goals that were deleted
   */
  async clearAllGoals(): Promise<number> {
    const goals = await db.goals
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .toArray();

    await db.goals
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();

    return goals.length;
  }
}
