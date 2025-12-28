import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Scratchpad with Title and Tags', () => {
  const sessionId = 'test-session';
  const threadId = 'test-thread';

  beforeEach(async () => {
    await db.open();
    await db.scratchpad.clear();
    await planningServer.switchContext?.({ sessionId, threadId });
  });

  it('should add scratchpad with title and tags', async () => {
    const result = await planningServer.callTool('addScratchpad', {
      title: 'Bug Report',
      note: 'User authentication fails on mobile',
      tags: ['bug', 'mobile', 'urgent'],
    });

    expect(result.isError).toBe(false);
    const content = result.content?.[0];
    if (content && content.type === 'text') {
      expect(content.text).toContain('Bug Report');
    }
    
    // Verify DB content
    const items = await db.scratchpad.toArray();
    expect(items).toHaveLength(1);
    expect(items[0].title).toBe('Bug Report');
    expect(items[0].tags).toEqual(['bug', 'mobile', 'urgent']);
  });

  it('should read scratchpad by tags', async () => {
    await planningServer.callTool('addScratchpad', {
      title: 'Note 1',
      note: 'Content 1',
      tags: ['bug'],
    });
    await planningServer.callTool('addScratchpad', {
      title: 'Note 2',
      note: 'Content 2',
      tags: ['feature'],
    });

    const result = await planningServer.callTool('listScratchpad', {
      tags: ['bug'],
    });

    expect(result.isError).toBe(false);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const data = result.structuredContent as any;
    expect(data.items).toHaveLength(1);
    expect(data.items[0].title).toBe('Note 1');
  });

  it('should display summary in context', async () => {
    await planningServer.callTool('addScratchpad', {
      title: 'Important Finding',
      note: 'Very long content that should not appear fully in context preview because it exceeds the character limit and will be truncated',
      tags: ['important'],
    });

    const context = await planningServer.getServiceContext?.({
      sessionId,
      threadId
    });

    if (!context) {
      throw new Error('Context is undefined');
    }

    expect(context.contextPrompt).toContain('Important Finding');
    expect(context.contextPrompt).toContain('[important]');
    // Content should be truncated to ~50 chars with "..."
    expect(context.contextPrompt).toContain('Very long content that should not appear fully');
    expect(context.contextPrompt).not.toContain('because it exceeds the character limit');
  });
  
  it('should handle capacity limits (MAX_NOTES=20)', async () => {
      // Add 21 items
      for (let i = 1; i <= 21; i++) {
          await planningServer.callTool('addScratchpad', {
              note: `Note ${i}`,
              title: `Title ${i}`
          });
      }
      
      const items = await db.scratchpad.toArray();
      expect(items).toHaveLength(20);
      
      // Should have removed the first one (Note 1)
      expect(items.find(i => i.content === 'Note 1')).toBeUndefined();
      // Should have the last one (Note 21)
      expect(items.find(i => i.content === 'Note 21')).toBeDefined();
  });
});
