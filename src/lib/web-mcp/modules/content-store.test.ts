import { describe, it, expect, beforeEach, vi } from 'vitest';
import { dbService, dbUtils } from '@/lib/db';
import { computeContentHash } from '@/lib/content-hash';
import fileStoreServer, {
  AddContentInput,
  AddContentOutput,
} from './content-store';

// Mock the database service
vi.mock('@/lib/db', () => ({
  dbService: {
    fileStores: {
      read: vi.fn(),
    },
    fileContents: {
      findByHashAndStore: vi.fn(),
      upsert: vi.fn(),
    },
    fileChunks: {
      upsertMany: vi.fn(),
    },
  },
  dbUtils: {
    getFileChunksByContent: vi.fn(),
    getFileChunksByStore: vi.fn(),
  },
}));

// Mock the content hash function
vi.mock('@/lib/content-hash', () => ({
  computeContentHash: vi.fn(),
}));

describe('Content Store - Duplicate Handling', () => {
  const mockStore1 = {
    id: 'store_1',
    name: 'Store 1',
    createdAt: new Date(),
    updatedAt: new Date(),
  };
  const mockStore2 = {
    id: 'store_2',
    name: 'Store 2',
    createdAt: new Date(),
    updatedAt: new Date(),
  };
  const testContent = 'This is test content for duplicate detection';
  const testHash = 'abc123def456';

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(computeContentHash).mockResolvedValue(testHash);
    vi.mocked(dbUtils.getFileChunksByStore).mockResolvedValue([]);
    vi.mocked(dbUtils.getFileChunksByContent).mockResolvedValue([]);
  });

  describe('Same content in different stores', () => {
    it('should allow the same content in different storeIds', async () => {
      // Mock store existence
      vi.mocked(dbService.fileStores.read).mockResolvedValue(mockStore1);

      // Mock no existing content in store1
      vi.mocked(dbService.fileContents.findByHashAndStore).mockResolvedValue(
        undefined,
      );

      // Mock successful storage
      vi.mocked(dbService.fileContents.upsert).mockResolvedValue();
      vi.mocked(dbService.fileChunks.upsertMany).mockResolvedValue();

      const input1: AddContentInput = {
        storeId: 'store_1',
        content: testContent,
        metadata: {
          filename: 'test.txt',
          mimeType: 'text/plain',
          size: testContent.length,
        },
      };

      const result1 = await fileStoreServer.callTool('addContent', input1);
      expect(result1.error).toBeUndefined();
      expect(vi.mocked(dbService.fileContents.upsert)).toHaveBeenCalledTimes(1);

      // Now add the same content to a different store
      vi.mocked(dbService.fileStores.read).mockResolvedValue(mockStore2);

      const input2: AddContentInput = {
        storeId: 'store_2',
        content: testContent,
        metadata: {
          filename: 'test.txt',
          mimeType: 'text/plain',
          size: testContent.length,
        },
      };

      const result2 = await fileStoreServer.callTool('addContent', input2);
      expect(result2.error).toBeUndefined();
      expect(vi.mocked(dbService.fileContents.upsert)).toHaveBeenCalledTimes(2);
    });
  });

  describe('Same content in the same store', () => {
    it('should prevent duplicate content within the same storeId', async () => {
      const existingContent = {
        id: 'existing_content_id',
        storeId: 'store_1',
        filename: 'existing.txt',
        mimeType: 'text/plain',
        size: testContent.length,
        uploadedAt: new Date(),
        content: testContent,
        lineCount: 1,
        summary: testContent,
        contentHash: testHash,
      };

      // Mock store existence
      vi.mocked(dbService.fileStores.read).mockResolvedValue(mockStore1);

      // Mock existing content found
      vi.mocked(dbService.fileContents.findByHashAndStore).mockResolvedValue(
        existingContent,
      );
      vi.mocked(dbUtils.getFileChunksByContent).mockResolvedValue([
        {
          id: 'chunk_1',
          contentId: 'existing_content_id',
          chunkIndex: 0,
          text: testContent,
          startLine: 1,
          endLine: 1,
        },
      ]);

      const input: AddContentInput = {
        storeId: 'store_1',
        content: testContent,
        metadata: {
          filename: 'duplicate.txt', // Different filename but same content
          mimeType: 'text/plain',
          size: testContent.length,
        },
      };

      const result = await fileStoreServer.callTool('addContent', input);
      expect(result.error).toBeUndefined();

      // Should not create new content
      expect(vi.mocked(dbService.fileContents.upsert)).not.toHaveBeenCalled();
      expect(vi.mocked(dbService.fileChunks.upsertMany)).not.toHaveBeenCalled();

      // Should return existing content information
      const resultContent = result.result as {
        content: Array<{ type: string; text: string }>;
      };
      const content = JSON.parse(
        resultContent.content[0].text,
      ) as AddContentOutput;
      expect(content.contentId).toBe('existing_content_id');
      expect(content.chunkCount).toBe(1);
    });
  });

  describe('Content hash calculation', () => {
    it('should call computeContentHash with the correct content', async () => {
      // Mock store existence
      vi.mocked(dbService.fileStores.read).mockResolvedValue(mockStore1);
      vi.mocked(dbService.fileContents.findByHashAndStore).mockResolvedValue(
        undefined,
      );
      vi.mocked(dbService.fileContents.upsert).mockResolvedValue();
      vi.mocked(dbService.fileChunks.upsertMany).mockResolvedValue();

      const input: AddContentInput = {
        storeId: 'store_1',
        content: testContent,
        metadata: {
          filename: 'test.txt',
          mimeType: 'text/plain',
          size: testContent.length,
        },
      };

      await fileStoreServer.callTool('addContent', input);

      expect(vi.mocked(computeContentHash)).toHaveBeenCalledWith(testContent);
      expect(
        vi.mocked(dbService.fileContents.findByHashAndStore),
      ).toHaveBeenCalledWith(testHash, 'store_1');
    });
  });

  describe('Store isolation', () => {
    it('should check for duplicates only within the specified store', async () => {
      // Mock store existence
      vi.mocked(dbService.fileStores.read).mockResolvedValue(mockStore1);
      vi.mocked(dbService.fileContents.findByHashAndStore).mockResolvedValue(
        undefined,
      );
      vi.mocked(dbService.fileContents.upsert).mockResolvedValue();
      vi.mocked(dbService.fileChunks.upsertMany).mockResolvedValue();

      const input: AddContentInput = {
        storeId: 'store_1',
        content: testContent,
        metadata: {
          filename: 'test.txt',
          mimeType: 'text/plain',
          size: testContent.length,
        },
      };

      await fileStoreServer.callTool('addContent', input);

      // Should only check for duplicates in store_1, not globally
      expect(
        vi.mocked(dbService.fileContents.findByHashAndStore),
      ).toHaveBeenCalledWith(testHash, 'store_1');
      expect(
        vi.mocked(dbService.fileContents.findByHashAndStore),
      ).toHaveBeenCalledTimes(1);
    });
  });
});
