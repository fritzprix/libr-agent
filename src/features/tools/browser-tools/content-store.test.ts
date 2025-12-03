import { describe, it, expect, beforeEach } from 'vitest';
import { ContentStore } from './content-store';

describe('ContentStore', () => {
    const sessionId = 'test-session';

    beforeEach(() => {
        // Reset store state if possible, or use unique session IDs
        // Since ContentStore is a singleton module with private state, we can't easily reset it.
        // We'll use unique session IDs for each test or rely on overwrites.
    });

    it('should paginate content correctly', () => {
        const content = 'a'.repeat(10000);
        const pageSize = 2000;
        const { totalPages, firstPage } = ContentStore.saveContent(sessionId, content, pageSize);

        expect(totalPages).toBe(5);
        expect(firstPage.length).toBe(pageSize);
        expect(firstPage).toBe('a'.repeat(pageSize));
    });

    it('should handle content smaller than page size', () => {
        const content = 'small content';
        const { totalPages, firstPage } = ContentStore.saveContent('small-session', content, 100);

        expect(totalPages).toBe(1);
        expect(firstPage).toBe(content);
    });

    it('should retrieve specific pages', () => {
        const content = '1234567890';
        const pageSize = 2;
        ContentStore.saveContent('page-session', content, pageSize);

        const page1 = ContentStore.getPage('page-session', 1);
        const page2 = ContentStore.getPage('page-session', 2);
        const page5 = ContentStore.getPage('page-session', 5);
        const page6 = ContentStore.getPage('page-session', 6);

        expect(page1?.content).toBe('12');
        expect(page2?.content).toBe('34');
        expect(page5?.content).toBe('90');
        expect(page6).toBeNull();
    });

    it('should return null for non-existent session', () => {
        const page = ContentStore.getPage('non-existent', 1);
        expect(page).toBeNull();
    });

    it('should return null for invalid page numbers', () => {
        ContentStore.saveContent('invalid-page-session', 'content');
        expect(ContentStore.getPage('invalid-page-session', 0)).toBeNull();
        expect(ContentStore.getPage('invalid-page-session', 2)).toBeNull();
    });
});
