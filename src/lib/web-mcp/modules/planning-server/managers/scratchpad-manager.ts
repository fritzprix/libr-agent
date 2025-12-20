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
      title: m.title,
      content: m.content,
      tags: m.tags,
      source: m.source,
    }));
  }

  /**
   * Adds a new scratchpad item. Automatically removes the oldest item if at capacity (20 items).
   *
   * @param note - The content of the scratchpad item
   * @param source - Optional source/origin of the note
   * @param title - Optional title for the note
   * @param tags - Optional tags for categorization
   * @returns MCPResult with updated scratchpad list and capacity warning if applicable
   */
  async addScratchpad(
    note: string,
    source?: string,
    title?: string,
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    await db.scratchpad.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: note,
      source,
      title,
      tags,
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
    if (title) {
      message = `Scratchpad ID:${newItemId} [${title}] added\n${capacityWarning}`;
    }
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
   * Reads scratchpad items by IDs or tags.
   *
   * @param ids - List of IDs to read
   * @param tags - List of tags to filter by
   * @returns MCPResult with the requested scratchpad items
   */
  async readScratchpad(
    ids?: number[],
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    let items: ScratchpadItem[] = [];
    const allItems = await this.getScratchpadList();

    if (ids && ids.length > 0) {
      items = allItems.filter((item) => ids.includes(item.id));
    } else if (tags && tags.length > 0) {
      items = allItems.filter(
        (item) =>
          Array.isArray(item.tags) &&
          item.tags.some((tag) => tags.includes(tag)),
      );
    } else {
      items = allItems;
    }

    // Format scratchpad items as readable text
    if (items.length === 0) {
      const filterDesc = ids
        ? `IDs: ${ids.join(', ')}`
        : tags
          ? `tags: ${tags.join(', ')}`
          : 'any criteria';
      return createMCPStructuredToolResult(
        `No scratchpad items found matching ${filterDesc}`,
        {
          success: false,
          scratchpad: [],
        },
      );
    }

    const textParts: string[] = [
      `Found ${items.length} scratchpad item(s):`,
      '',
    ];

    items.forEach((item) => {
      const header = item.title
        ? `[${item.title}]`
        : `Scratchpad ID:${item.id}`;
      const tagsPart =
        Array.isArray(item.tags) && item.tags.length > 0
          ? ` (tags: ${item.tags.join(', ')})`
          : '';
      const sourcePart = item.source ? `\nSource: ${item.source}` : '';

      textParts.push(`## ${header}${tagsPart}`);
      if (sourcePart) {
        textParts.push(sourcePart);
      }
      textParts.push(item.content);
      textParts.push(''); // Blank line between items
    });

    return createMCPStructuredToolResult(textParts.join('\n'), {
      success: true,
      scratchpad: items,
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
