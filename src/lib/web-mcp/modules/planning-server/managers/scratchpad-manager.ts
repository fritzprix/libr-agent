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
   * Lists scratchpad items with pagination and filtering.
   * Returns metadata and content preview.
   */
  async listScratchpad(
    page: number = 1,
    pageSize: number = 10,
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { items: ScratchpadItem[] }>> {
    let items = await this.getScratchpadList();

    // Filter by tags if provided
    if (tags && tags.length > 0) {
      items = items.filter(
        (item) =>
          Array.isArray(item.tags) &&
          item.tags.some((tag) => tags.includes(tag)),
      );
    }

    const totalItems = items.length;
    const totalPages = Math.ceil(totalItems / pageSize);
    const start = (page - 1) * pageSize;
    const paginatedItems = items.slice(start, start + pageSize);

    if (paginatedItems.length === 0) {
      return createMCPStructuredToolResult(
        `No scratchpad items found${tags ? ` matching tags: ${tags.join(', ')}` : ''}.`,
        {
          success: true,
          items: [],
          pagination: { page, pageSize, totalItems, totalPages },
        },
      );
    }

    const textParts: string[] = [
      `Scratchpad List (Page ${page}/${totalPages}, Total: ${totalItems})`,
      '',
    ];

    paginatedItems.forEach((item) => {
      const titlePart = item.title ? `[${item.title}]` : '';
      const tagsPart =
        Array.isArray(item.tags) && item.tags.length > 0
          ? ` (tags: ${item.tags.join(', ')})`
          : '';
      // Preview content (first 50 chars)
      const contentPreview =
        item.content.length > 50
          ? item.content.slice(0, 50) + '...'
          : item.content;

      textParts.push(
        `- ID:${item.id} ${titlePart} ${contentPreview}${tagsPart}`,
      );
    });

    return createMCPStructuredToolResult(textParts.join('\n'), {
      success: true,
      items: paginatedItems,
      pagination: { page, pageSize, totalItems, totalPages },
    });
  }

  /**
   * Reads scratchpad items by IDs.
   *
   * @param ids - List of IDs to read (Required)
   * @returns MCPResult with the requested scratchpad items
   */
  async readScratchpad(
    ids: number[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    if (!ids || ids.length === 0) {
      return createMCPStructuredToolResult(
        'Error: "ids" parameter is required. Use listScratchpad to find IDs first.',
        { success: false, scratchpad: [] },
      );
    }

    const allItems = await this.getScratchpadList();
    const items = allItems.filter((item) => ids.includes(item.id));

    // Format scratchpad items as readable text
    if (items.length === 0) {
      return createMCPStructuredToolResult(
        `No scratchpad items found matching IDs: ${ids.join(', ')}`,
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
