import { getLogger } from '@/lib/logger';

const logger = getLogger('ContentStore');

interface ContentPage {
  content: string;
  pageNumber: number;
  totalPages: number;
}

interface ContentSession {
  pages: string[];
  timestamp: number;
  url?: string;
  title?: string;
}

// In-memory store: sessionId -> ContentSession
const contentStore = new Map<string, ContentSession>();

// Clean up old sessions periodically (e.g., every 1 hour)
const CLEANUP_INTERVAL = 60 * 60 * 1000;
const MAX_AGE = 24 * 60 * 60 * 1000; // 24 hours

setInterval(() => {
  const now = Date.now();
  for (const [id, session] of contentStore.entries()) {
    if (now - session.timestamp > MAX_AGE) {
      contentStore.delete(id);
    }
  }
}, CLEANUP_INTERVAL);

export const ContentStore = {
  saveContent: (
    sessionId: string,
    content: string,
    pageSize: number = 6000,
    autoMerge: boolean = false,
  ) => {
    // Strict Line-Based Chunking (Overflow Allowed)
    const pages: string[] = [];
    const lines = content.split('\n');
    let currentPage = '';

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const isLastLine = i === lines.length - 1;
      // Add newline back to line unless it's the very last line of content (split removes them)
      // Actually, we can just append '\n' when adding to page, or better yet:
      // constructing the page string from lines.

      const lineWithNewline = isLastLine ? line : line + '\n';

      // If current page + new line fits, add it
      if (currentPage.length + lineWithNewline.length <= pageSize) {
        currentPage += lineWithNewline;
      } else {
        // Doesn't fit. 

        // Case A: Current page has content. Push it, start new page with this line.
        if (currentPage.length > 0) {
          pages.push(currentPage);
          currentPage = lineWithNewline;
        }
        // Case B: Current page is empty (this single line is > pageSize).
        // Push it immediately (Overflow).
        else {
          pages.push(lineWithNewline);
          currentPage = '';
        }
      }
    }

    if (currentPage.length > 0) {
      pages.push(currentPage);
    }

    if (pages.length === 0) {
      pages.push('');
    }

    // Auto-merge logic: if autoMerge is enabled and content meets criteria
    const shouldAutoMerge =
      autoMerge && (pages.length <= 2 || content.length < 5000);
    const mergedContent = shouldAutoMerge ? content : null;

    contentStore.set(sessionId, {
      pages,
      timestamp: Date.now(),
    });

    logger.debug('Content saved', { sessionId, totalPages: pages.length });

    return {
      totalPages: pages.length,
      firstPage: pages[0],
      mergedContent,
      autoMerged: shouldAutoMerge,
    };
  },

  getPage: (sessionId: string, page: number): ContentPage | null => {
    const session = contentStore.get(sessionId);
    if (!session) {
      return null;
    }

    // 1-based index
    const pageIndex = page - 1;
    if (pageIndex < 0 || pageIndex >= session.pages.length) {
      return null;
    }

    return {
      content: session.pages[pageIndex],
      pageNumber: page,
      totalPages: session.pages.length,
    };
  },

  hasContent: (sessionId: string): boolean => {
    return contentStore.has(sessionId);
  },
};
