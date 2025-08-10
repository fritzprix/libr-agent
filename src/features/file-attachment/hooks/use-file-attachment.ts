import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { AttachmentReference } from '@/models/chat';
import { AddContentOutput } from '@/lib/web-mcp/modules/file-store';
import { getLogger } from '@/lib/logger';
import { SearchResult } from '@/models/search-engine';

const logger = getLogger('useFileAttachment');

export function useFileStore() {
  // ... store management logic
}

export function useFileUpload() {
  const { executeCall } = useWebMCPTools();

  const uploadFile = async (
    file: File,
    storeId: string,
  ): Promise<AttachmentReference> => {
    try {
      const content = await file.text();
      const result = (await executeCall('file-store', 'addContent', {
        storeId,
        content,
        metadata: {
          filename: file.name,
          mimeType: file.type,
          size: file.size,
          uploadedAt: new Date().toISOString(),
        },
      })) as AddContentOutput;

      const attachment: AttachmentReference = {
        storeId: result.storeId,
        contentId: result.contentId,
        filename: result.filename,
        mimeType: result.mimeType,
        size: result.size,
        lineCount: result.lineCount,
        preview: result.preview,
        uploadedAt: result.uploadedAt.toISOString(),
        chunkCount: result.chunkCount,
        lastAccessedAt: new Date().toISOString(),
      };

      logger.info('File uploaded successfully', {
        contentId: attachment.contentId,
        filename: attachment.filename,
        chunkCount: attachment.chunkCount,
      });

      return attachment;
    } catch (error) {
      logger.error('File upload failed', { filename: file.name, error });
      throw error;
    }
  };

  return { uploadFile };
}

export function useFileSearch() {
  const { executeCall } = useWebMCPTools();

  const searchContent = async (query: string, storeId: string) => {
    try {
      const { results } = (await executeCall('file-store', 'similaritySearch', {
        storeId,
        query,
        options: { topN: 5, threshold: 0.5 },
      })) as { results: SearchResult[] };
      return results;
    } catch (error) {
      logger.error('Content search failed', { query, error });
      throw error;
    }
  };

  return { searchContent };
}
