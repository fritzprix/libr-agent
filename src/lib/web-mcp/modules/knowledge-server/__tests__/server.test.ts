import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import knowledgeServer from '../server';

interface SaveKnowledgeResult {
    id: number;
    title: string;
    tags: string[];
}

interface SearchKnowledgeResult {
    results: Array<{
        id: number;
        title: string;
        tags: string[];
        preview: string;
    }>;
}

interface ListKnowledgeResult {
    results: Array<{
        id: number;
        title: string;
        tags: string[];
        preview: string;
        updatedAt: number;
    }>;
    total: number;
    offset: number;
    limit: number;
}

describe('Knowledge Server', () => {
    const assistantId = 'test-assistant';

    beforeEach(async () => {
        await db.knowledge.clear();
        await knowledgeServer.switchContext!({ assistantId });
    });

    afterEach(async () => {
        // await db.close();
    });

    it('should save knowledge', async () => {
        const result = await knowledgeServer.callTool('save_knowledge', {
            title: 'Test Title',
            content: 'Test Content',
            tags: ['tag1', 'tag2'],
        });

        expect(result.isError).toBe(false);
        const { id } = result.structuredContent as SaveKnowledgeResult;
        expect(id).toBeDefined();

        const saved = await db.knowledge.get(id);
        expect(saved).toBeDefined();
        expect(saved?.title).toBe('Test Title');
        expect(saved?.tags).toEqual(['tag1', 'tag2']);
    });

    it('should search knowledge by tag', async () => {
        await knowledgeServer.callTool('save_knowledge', {
            title: 'React Hooks',
            content: 'useEffect is cool',
            tags: ['react', 'hooks'],
        });
        await knowledgeServer.callTool('save_knowledge', {
            title: 'Vue Composition',
            content: 'setup is cool',
            tags: ['vue', 'composition'],
        });

        const result = await knowledgeServer.callTool('search_knowledge', {
            tags: ['react'],
        });

        const { results } = result.structuredContent as SearchKnowledgeResult;
        expect(results).toHaveLength(1);
        expect(results[0].title).toBe('React Hooks');
    });

    it('should search knowledge by query', async () => {
        await knowledgeServer.callTool('save_knowledge', {
            title: 'Rust Macros',
            content: 'macro_rules! is powerful',
            tags: ['rust'],
        });

        const result = await knowledgeServer.callTool('search_knowledge', {
            query: 'macro',
        });

        const { results } = result.structuredContent as SearchKnowledgeResult;
        expect(results).toHaveLength(1);
        expect(results[0].title).toBe('Rust Macros');
    });

    it('should isolate knowledge between assistants', async () => {
        // Assistant 1
        await knowledgeServer.switchContext!({ assistantId: 'assistant1' });
        await knowledgeServer.callTool('save_knowledge', {
            title: 'Secret 1',
            content: 'Content 1',
        });

        // Assistant 2
        await knowledgeServer.switchContext!({ assistantId: 'assistant2' });
        await knowledgeServer.callTool('save_knowledge', {
            title: 'Secret 2',
            content: 'Content 2',
        });

        // Verify Assistant 1 can't see Secret 2
        await knowledgeServer.switchContext!({ assistantId: 'assistant1' });
        const result1 = await knowledgeServer.callTool('search_knowledge', {});
        const { results: items1 } = result1.structuredContent as SearchKnowledgeResult;
        expect(items1).toHaveLength(1);
        expect(items1[0].title).toBe('Secret 1');

        // Verify Assistant 2 can't see Secret 1
        await knowledgeServer.switchContext!({ assistantId: 'assistant2' });
        const result2 = await knowledgeServer.callTool('search_knowledge', {});
        const { results: items2 } = result2.structuredContent as SearchKnowledgeResult;
        expect(items2).toHaveLength(1);
        expect(items2[0].title).toBe('Secret 2');
    });

    it('should list knowledge items', async () => {
        await knowledgeServer.switchContext!({ assistantId: 'test-assistant' });

        // Create multiple items
        for (let i = 1; i <= 5; i++) {
            await knowledgeServer.callTool('save_knowledge', {
                title: `Item ${i}`,
                content: `Content ${i}`,
                tags: ['list-test'],
            });
        }

        const result = await knowledgeServer.callTool('list_knowledge', { limit: 3 });
        const { results, total } = result.structuredContent as ListKnowledgeResult;

        expect(total).toBeGreaterThanOrEqual(5);
        expect(results).toHaveLength(3);
        // Should be newest first
        expect(results[0].title).toBe('Item 5');
    });

    it('should delete knowledge', async () => {
        const saveResult = await knowledgeServer.callTool('save_knowledge', {
            title: 'To Delete',
            content: 'Delete me',
        });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const id = (saveResult.structuredContent as any).id;

        await knowledgeServer.callTool('delete_knowledge', { id });

        const item = await db.knowledge.get(id);
        expect(item).toBeUndefined();
    });
});
