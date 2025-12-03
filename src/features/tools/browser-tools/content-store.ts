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
  ) => {
    const pages: string[] = [];

    // Simple pagination by character count (approx. token count)
    // 2048 tokens * ~4 chars/token = ~8192 chars. Using 6000 for safety.
    for (let i = 0; i < content.length; i += pageSize) {
      pages.push(content.slice(i, i + pageSize));
    }

    if (pages.length === 0) {
      pages.push('');
    }

    contentStore.set(sessionId, {
      pages,
      timestamp: Date.now(),
    });

    logger.debug('Content saved', { sessionId, totalPages: pages.length });

    return {
      totalPages: pages.length,
      firstPage: pages[0],
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
