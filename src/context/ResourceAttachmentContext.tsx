import React, {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ResourceAttachmentContext');

interface ResourceAttachmentContextType {
  files: AttachmentReference[];
  addFile: (url: string, mimeType: string, filename?: string) => Promise<AttachmentReference>;
  removeFile: (ref: AttachmentReference) => Promise<void>;
  clearFiles: () => void;
  isLoading: boolean;
  getFileById: (id: string) => AttachmentReference | undefined;
}

const ResourceAttachmentContext = createContext<ResourceAttachmentContextType | undefined>(
  undefined,
);

interface ResourceAttachmentProviderProps {
  children: ReactNode;
}

export const ResourceAttachmentProvider: React.FC<ResourceAttachmentProviderProps> = ({
  children,
}) => {
  const [files, setFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Helper function to extract filename from URL
  const extractFilenameFromUrl = useCallback((url: string): string => {
    try {
      const urlObj = new URL(url);
      const pathname = urlObj.pathname;
      const filename = pathname.split('/').pop() || 'unknown_file';
      return filename;
    } catch {
      return `file_${Date.now()}`;
    }
  }, []);

  // Helper function to generate preview content
  const generatePreview = useCallback(async (url: string, mimeType: string): Promise<string> => {
    try {
      // For text files, try to fetch and preview first few lines
      if (mimeType.startsWith('text/')) {
        // This would typically use MCP tools to read file content
        // For now, return a placeholder
        return 'Preview will be generated when file is processed...';
      }
      return `${mimeType} file - Preview not available`;
    } catch (error) {
      logger.warn('Failed to generate preview', { url, mimeType, error });
      return 'Preview not available';
    }
  }, []);

  const addFile = useCallback(
    async (url: string, mimeType: string, filename?: string): Promise<AttachmentReference> => {
      setIsLoading(true);
      try {
        logger.debug('Adding file attachment', { url, mimeType, filename });

        const actualFilename = filename || extractFilenameFromUrl(url);
        const preview = await generatePreview(url, mimeType);
        
        // Create a new attachment reference
        const attachment: AttachmentReference = {
          storeId: `store_${Date.now()}`,
          contentId: `content_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
          filename: actualFilename,
          mimeType,
          size: 0, // Will be updated when file is actually processed
          lineCount: 0, // Will be updated when file is processed
          preview,
          uploadedAt: new Date().toISOString(),
          chunkCount: 0,
          lastAccessedAt: new Date().toISOString(),
        };

        // Check if file already exists (by URL/filename)
        const existingFile = files.find(
          (file) => file.filename === actualFilename && file.storeId.includes(url)
        );

        if (existingFile) {
          logger.warn('File already exists', { filename: actualFilename });
          return existingFile;
        }

        // Add to files array
        setFiles((prevFiles) => [...prevFiles, attachment]);
        
        logger.info('File attachment added successfully', { 
          filename: actualFilename, 
          contentId: attachment.contentId 
        });

        return attachment;
      } catch (error) {
        logger.error('Failed to add file attachment', { url, mimeType, error });
        throw new Error(`Failed to add file: ${error instanceof Error ? error.message : 'Unknown error'}`);
      } finally {
        setIsLoading(false);
      }
    },
    [files, extractFilenameFromUrl, generatePreview],
  );

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      try {
        logger.debug('Removing file attachment', { contentId: ref.contentId, filename: ref.filename });

        setFiles((prevFiles) => 
          prevFiles.filter((file) => file.contentId !== ref.contentId)
        );

        logger.info('File attachment removed successfully', { 
          filename: ref.filename, 
          contentId: ref.contentId 
        });
      } catch (error) {
        logger.error('Failed to remove file attachment', { ref, error });
        throw new Error(`Failed to remove file: ${error instanceof Error ? error.message : 'Unknown error'}`);
      }
    },
    [],
  );

  const clearFiles = useCallback(() => {
    logger.debug('Clearing all file attachments', { count: files.length });
    setFiles([]);
    logger.info('All file attachments cleared');
  }, [files.length]);

  const getFileById = useCallback(
    (id: string): AttachmentReference | undefined => {
      return files.find((file) => file.contentId === id || file.storeId === id);
    },
    [files],
  );

  const contextValue: ResourceAttachmentContextType = useMemo(
    () => ({
      files,
      addFile,
      removeFile,
      clearFiles,
      isLoading,
      getFileById,
    }),
    [files, addFile, removeFile, clearFiles, isLoading, getFileById],
  );

  return (
    <ResourceAttachmentContext.Provider value={contextValue}>
      {children}
    </ResourceAttachmentContext.Provider>
  );
};

// Custom hook to use the ResourceAttachmentContext
export const useResourceAttachment = () => {
  const context = useContext(ResourceAttachmentContext);
  if (context === undefined) {
    throw new Error('useResourceAttachment must be used within a ResourceAttachmentProvider');
  }
  return context;
};

export { ResourceAttachmentContext };