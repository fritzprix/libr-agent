import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { db } from './db';
import { knowledgeTools as tools } from './tools';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';

class KnowledgeManager {
  private assistantId: string | null = null;

  setContext(assistantId: string) {
    this.assistantId = assistantId;
  }

  private checkContext() {
    if (!this.assistantId) {
      throw new Error('Assistant context not set. Use switchContext first.');
    }
  }

  async saveKnowledge(
    title: string,
    content: string,
    tags: string[] = [],
  ): Promise<MCPResult> {
    this.checkContext();
    const now = Date.now();
    const id = await db.knowledge.add({
      assistantId: this.assistantId!,
      title,
      content,
      tags,
      createdAt: now,
      updatedAt: now,
    });

    // Get database stats for context
    const totalItems = await db.knowledge
      .where('assistantId')
      .equals(this.assistantId!)
      .count();

    const nextActions: string[] = [
      'Use search_knowledge to find this item later',
      'Use read_knowledge to view full content',
    ];

    const suggestions: string[] = [];
    if (tags.length > 0) {
      suggestions.push(
        `Search by tags: search_knowledge with tags: [${tags.map((t) => `"${t}"`).join(', ')}]`,
      );
    }
    suggestions.push(`Search by title or content keywords from: "${title}"`);

    let message = `Knowledge saved: "${title}" (ID: ${id})`;
    if (tags.length > 0) {
      message += `\nTags: ${tags.join(', ')}`;
    }
    message += `\n\nTotal knowledge items: ${totalItems}`;

    return new MCPResponseBuilder({
      id,
      title,
      tags,
      totalItems,
    })
      .withMessage(message)
      .withNextActions(nextActions)
      .withSuggestions(suggestions)
      .asSuccess();
  }

  async searchKnowledge(query?: string, tags?: string[]): Promise<MCPResult> {
    this.checkContext();

    let collection = db.knowledge
      .where('assistantId')
      .equals(this.assistantId!);

    // If tags are provided, use them for filtering (Dexie multiEntry)
    // Note: Dexie doesn't support complex compound queries with multiEntry easily in one go
    // combined with other indices without some manual work.
    // For simplicity and "Assistant Scope", we filter by assistantId first.
    // Then we filter in memory or use Dexie's collection methods.

    // Optimization: If tags are present, we could potentially use the tags index
    // but we need to ensure we only get items for THIS assistant.
    // Since assistantId is not part of the tags index, we stick to assistantId index
    // and filter manually. For < 10k items per assistant, this is fast enough.

    let results = await collection.toArray();
    const totalItems = results.length;

    if (tags && tags.length > 0) {
      results = results.filter((item) =>
        tags.every((tag) => item.tags.includes(tag)),
      );
    }

    if (query) {
      const lowerQuery = query.toLowerCase();
      results = results.filter(
        (item) =>
          item.title.toLowerCase().includes(lowerQuery) ||
          item.content.toLowerCase().includes(lowerQuery),
      );
    }

    const summary = results.map((item) => ({
      id: item.id,
      title: item.title,
      tags: item.tags,
      preview:
        item.content.slice(0, 100) + (item.content.length > 100 ? '...' : ''),
    }));

    // Handle no results with improved guidance
    if (results.length === 0) {
      const suggestions: string[] = [];

      if (query && tags && tags.length > 0) {
        suggestions.push('Try search with query only (remove tags filter)');
        suggestions.push('Try search with tags only (remove query)');
      } else if (query) {
        suggestions.push('Try shorter or different keywords');
        suggestions.push('Use list_knowledge to browse all items');
      } else if (tags && tags.length > 0) {
        suggestions.push('Try fewer tags or different tag combinations');
        suggestions.push('Use list_knowledge to see available items and tags');
      }

      if (totalItems < 20) {
        suggestions.push('Use list_knowledge to browse all items');
      }

      let message = 'No knowledge items found';
      if (query && tags && tags.length > 0) {
        message += ` matching query "${query}" with tags [${tags.join(', ')}]`;
      } else if (query) {
        message += ` matching query "${query}"`;
      } else if (tags && tags.length > 0) {
        message += ` with tags [${tags.join(', ')}]`;
      }
      message += `.\n\nTotal knowledge items: ${totalItems}`;

      return new MCPResponseBuilder({
        results: [],
        query,
        tags,
        totalItems,
        suggestions,
      })
        .withMessage(message)
        .withSuggestions(suggestions)
        .asSuccess();
    }

    // Success with results
    let message = `Found ${results.length} knowledge item(s)`;
    if (query && tags && tags.length > 0) {
      message += ` matching "${query}" with tags [${tags.join(', ')}]`;
    } else if (query) {
      message += ` matching "${query}"`;
    } else if (tags && tags.length > 0) {
      message += ` with tags [${tags.join(', ')}]`;
    }

    const nextActions: string[] = ['Use read_knowledge to view full content'];
    if (results.length > 5) {
      nextActions.push('Refine search with more specific keywords or tags');
    }

    return new MCPResponseBuilder({
      results: summary,
      query,
      tags,
      totalItems,
    })
      .withMessage(message)
      .withNextActions(nextActions)
      .asSuccess();
  }

  async listKnowledge(limit = 50, offset = 0): Promise<MCPResult> {
    this.checkContext();

    const count = await db.knowledge
      .where('assistantId')
      .equals(this.assistantId!)
      .count();

    const items = await db.knowledge
      .where('assistantId')
      .equals(this.assistantId!)
      .reverse() // Newest first (by ID/insertion order roughly, or add sortBy('updatedAt'))
      .offset(offset)
      .limit(limit)
      .toArray();

    const summary = items.map((item) => ({
      id: item.id,
      title: item.title,
      tags: item.tags,
      preview:
        item.content.slice(0, 100) + (item.content.length > 100 ? '...' : ''),
      updatedAt: item.updatedAt,
    }));

    return createMCPStructuredToolResult(
      `Listing ${summary.length} items (Total: ${count})`,
      { results: summary, total: count, offset, limit },
    );
  }

  async readKnowledge(id: number): Promise<MCPResult> {
    this.checkContext();
    const item = await db.knowledge.get(id);

    if (!item || item.assistantId !== this.assistantId) {
      const suggestions: string[] = [
        'Use search_knowledge to find items by keywords or tags',
        'Use list_knowledge to browse all available items',
      ];

      return new MCPResponseBuilder({
        requestedId: id,
        suggestions,
      })
        .withMessage(`Knowledge item ${id} not found.`)
        .withSuggestions(suggestions)
        .asError(WebMCPErrorCodes.KNOWLEDGE.ITEM_NOT_FOUND);
    }

    return createMCPStructuredToolResult(
      `Reading knowledge: "${item.title}"`,
      item,
    );
  }

  async deleteKnowledge(id: number): Promise<MCPResult> {
    this.checkContext();
    const item = await db.knowledge.get(id);

    if (!item || item.assistantId !== this.assistantId) {
      const suggestions: string[] = [
        'Use search_knowledge to find items by keywords or tags',
        'Use list_knowledge to browse all available items',
      ];

      return new MCPResponseBuilder({
        requestedId: id,
        suggestions,
      })
        .withMessage(`Knowledge item ${id} not found.`)
        .withSuggestions(suggestions)
        .asError(WebMCPErrorCodes.KNOWLEDGE.ITEM_NOT_FOUND);
    }

    await db.knowledge.delete(id);

    const remainingItems = await db.knowledge
      .where('assistantId')
      .equals(this.assistantId!)
      .count();

    return new MCPResponseBuilder({
      deleted: true,
      id,
      title: item.title,
      remainingItems,
    })
      .withMessage(
        `Knowledge item deleted: "${item.title}" (ID: ${id})\n\n` +
        `Remaining knowledge items: ${remainingItems}`,
      )
      .asSuccess();
  }
}

const manager = new KnowledgeManager();

const knowledgeServer: WebMCPServer = {
  name: 'knowledge',
  displayName: 'Knowledge Base',
  description: 'Long-term memory for agents',
  version: '1.1.0',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResult<unknown>> {
    const typedArgs = (args as Record<string, unknown>) || {};

    // Auto-set context if provided (though switchContext is preferred)
    if (typeof typedArgs.assistantId === 'string') {
      manager.setContext(typedArgs.assistantId);
    }

    try {
      switch (name) {
        case 'save_knowledge':
          return await manager.saveKnowledge(
            typedArgs.title as string,
            typedArgs.content as string,
            typedArgs.tags as string[],
          );
        case 'search_knowledge':
          return await manager.searchKnowledge(
            typedArgs.query as string | undefined,
            typedArgs.tags as string[] | undefined,
          );
        case 'list_knowledge':
          return await manager.listKnowledge(
            typedArgs.limit as number | undefined,
            typedArgs.offset as number | undefined,
          );
        case 'read_knowledge':
          return await manager.readKnowledge(typedArgs.id as number);
        case 'delete_knowledge':
          return await manager.deleteKnowledge(typedArgs.id as number);
        default:
          return createMCPErrorToolResult(`Unknown tool: ${name}`);
      }
    } catch (error) {
      return createMCPErrorToolResult(
        error instanceof Error ? error.message : String(error),
      );
    }
  },

  async switchContext(options: ServiceContextOptions): Promise<void> {
    // We use assistantId as the primary context key
    // If assistantId is not provided, we might fallback to 'default' or throw
    // But for now, let's assume the UI/Host always sends it if available.
    const assistantId = options.assistantId || 'default';
    manager.setContext(assistantId);
  },

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<unknown>> {
    const assistantId = options?.assistantId || 'default';
    manager.setContext(assistantId);

    // Maybe return a summary of available knowledge?
    // For now, just a static message.
    return {
      contextPrompt: `Knowledge Base connected for assistant: ${assistantId}. Use search_knowledge to find information.`,
      structuredState: { assistantId },
    };
  },
};

export default knowledgeServer;
