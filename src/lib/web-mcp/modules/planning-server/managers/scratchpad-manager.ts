import { createMCPStructuredToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import { db } from '../db';
import type { ScratchpadItem, BaseOutput } from '../types';

const MAX_NOTES = 20;

/**
 * Manages scratchpad items (temporary notes and findings) with automatic capacity management.
 * Enforces a maximum of 20 notes, automatically removing the oldest when capacity is reached.
 *
 * @internal
 */
export class ScratchpadManager {
  constructor(
    private sessionId: string,
    private threadId: string,
  ) {}

  /**
   * Retrieves all scratchpad items for the current session/thread, sorted by ID.
   *
   * @returns Array of scratchpad items
   */
  async getScratchpadList(): Promise<ScratchpadItem[]> {
    const items = await db.scratchpad
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('id');

    return items.map((m) => ({
      id: m.id!,
      content: m.content,
      source: m.source,
    }));
  }

  /**
   * Adds a new scratchpad item. Automatically removes the oldest item if at capacity (20 items).
   *
   * @param note - The content of the scratchpad item
   * @param source - Optional source/origin of the note
   * @returns MCPResult with updated scratchpad list and capacity warning if applicable
   */
  async addScratchpad(
    note: string,
    source?: string,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    await db.scratchpad.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: note,
      source,
      createdAt: Date.now(),
    });

    // Enforce MAX_NOTES
    const items = await this.getScratchpadList();
    if (items.length > MAX_NOTES) {
      // Remove oldest
      const oldest = items[0]; // Sorted by id (auto-inc)
      await db.scratchpad.delete(oldest.id);
    }

    const updatedItems = await this.getScratchpadList();
    const capacityWarning =
      updatedItems.length === MAX_NOTES
        ? `⚠️ At capacity (${MAX_NOTES}/${MAX_NOTES}) - oldest items will be removed`
        : `Scratchpad: ${updatedItems.length}/${MAX_NOTES}`;

    // Get the ID of the newly added item (last one)
    const newItemId = updatedItems[updatedItems.length - 1].id;

    let message = `Scratchpad ID:${newItemId} added\n${capacityWarning}`;
    if (source) {
      message += `\nSource: ${source}`;
    }

    return createMCPStructuredToolResult<
      BaseOutput & { scratchpad: ScratchpadItem[] }
    >(message, {
      success: true,
      scratchpad: updatedItems,
    });
  }

  /**
   * Removes a specific scratchpad item by ID.
   *
   * @param id - The ID of the scratchpad item to remove
   * @returns MCPResult with updated scratchpad list
   */
  async clearScratchpad(
    id: number,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    const item = await db.scratchpad.get(id);
    if (
      !item ||
      item.sessionId !== this.sessionId ||
      item.threadId !== this.threadId
    ) {
      const scratchpad = await this.getScratchpadList();
      const validIds = scratchpad.map((m) => m.id);
      return createMCPStructuredToolResult<
        BaseOutput & { scratchpad: ScratchpadItem[] }
      >(
        `Scratchpad ID:${id} not found.\nValid IDs: ${validIds.length > 0 ? validIds.join(', ') : '(no scratchpad items)'}`,
        { success: false, scratchpad },
      );
    }

    await db.scratchpad.delete(id);
    const scratchpad = await this.getScratchpadList();

    return createMCPStructuredToolResult<
      BaseOutput & { scratchpad: ScratchpadItem[] }
    >(
      `Scratchpad ID:${id} cleared: "${item.content}"\nRemaining scratchpad items: ${scratchpad.length}`,
      { success: true, scratchpad },
    );
  }

  /**
   * Removes all scratchpad items for the current session/thread.
   *
   * @returns The number of items that were cleared
   */
  async clearAllScratchpad(): Promise<number> {
    const items = await this.getScratchpadList();
    const count = items.length;

    await db.scratchpad
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();

    return count;
  }
}
