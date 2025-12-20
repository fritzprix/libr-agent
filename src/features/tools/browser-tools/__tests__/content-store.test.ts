import { ContentStore } from '../content-store';
import { describe, it, expect, vi } from 'vitest';

// Mock logger
vi.mock('@/lib/logger', () => ({
    getLogger: () => ({
        debug: vi.fn(),
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
    }),
}));

describe('ContentStore Line-Based Chunking', () => {
    it('should split at newlines only', () => {
        const line1 = 'Line 1 content';
        const line2 = 'Line 2 content';
        const text = `${line1}\n${line2}`; // ~29 chars

        // Page size 20. 
        // Line 1 (14) fits. 
        // Line 1 + Line 2 (29) > 20.
        // Should split after Line 1.
        const result = ContentStore.saveContent('session1', text, 20);

        expect(result.firstPage).toBe(line1 + '\n');
        expect(result.totalPages).toBe(2);
    });

    it('should allow overflow for long lines', () => {
        const longLine = 'a'.repeat(50);
        // Page size 20.
        // Line is 50. Should NOT split. Should overflow.
        const result = ContentStore.saveContent('session2', longLine, 20);

        expect(result.firstPage).toBe(longLine);
        expect(result.totalPages).toBe(1);
    });

    it('should split distinct lines gracefully', () => {
        const l1 = 'a'.repeat(15);
        const l2 = 'b'.repeat(15);
        const l3 = 'c'.repeat(15);
        const text = `${l1}\n${l2}\n${l3}`;

        // Page size 20.
        // Page 1: l1 (16 w/ \n).
        // Page 2: l2 (16 w/ \n).
        // Page 3: l3 (15).
        const result = ContentStore.saveContent('session3', text, 20);

        expect(result.totalPages).toBe(3);
        expect(result.firstPage).toBe(l1 + '\n');
    });
});
