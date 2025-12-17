import { createPage } from '@/lib/db/crud';
import type { MCPServerEntity } from '@/models/chat';
import type { ListServersOutput } from '../types';

/**
 * Normalize pagination including pageSize=-1 for all results
 */
export function normalizePagination(
  items: MCPServerEntity[],
  page: number,
  pageSize: number,
): ListServersOutput {
  if (pageSize === -1) {
    // Return all items on page 1
    return {
      items,
      page: 1,
      pageSize: items.length,
      totalPages: 1,
      totalItems: items.length,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  // Use standard pagination helper
  return createPage(items, page, pageSize, items.length);
}
