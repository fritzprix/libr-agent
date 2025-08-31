# Resource Attachment System - Refactored Architecture

## Overview

This refactored architecture provides a clean, scalable foundation for handling file attachments in the SynapticFlow application with full MCP (Model Context Protocol) integration capabilities.

## Architecture Components

### 1. Core Context: `ResourceAttachmentContext.tsx`

**Purpose**: Centralized state management for file attachments using React Context API.

**Key Features**:

- State management for attached files
- Basic file operations (add, remove, clear)
- File lookup by ID
- Loading state management
- Proper TypeScript typing

**Interface**:

```typescript
interface ResourceAttachmentContextType {
  files: AttachmentReference[];
  addFile: (
    url: string,
    mimeType: string,
    filename?: string,
  ) => Promise<AttachmentReference>;
  removeFile: (ref: AttachmentReference) => Promise<void>;
  clearFiles: () => void;
  isLoading: boolean;
  getFileById: (id: string) => AttachmentReference | undefined;
}
```

### 2. Service Layer: `ResourceAttachmentService.ts`

**Purpose**: Business logic layer that handles actual file processing using MCP tools.

**Key Features**:

- File processing with MCP integration points
- Support for local files and web URLs
- Content preview generation
- File chunking for large documents
- Search within file content
- Extensible for different file types

**Main Methods**:

- `processFile()` - Process and create attachment references
- `getFileContent()` - Retrieve full file content
- `searchInFile()` - Search within specific files
- Helper methods for file type detection

### 3. Enhanced Hook: `useResourceAttachmentService.ts`

**Purpose**: High-level interface combining context state with service layer operations.

**Key Features**:

- Combines context state management with service operations
- File processing with full MCP integration
- Search across all attached files
- File statistics and analytics
- Error handling and logging

**Enhanced Methods**:

- `addAndProcessFile()` - Add and fully process files
- `getFileContent()` - Get full content using service layer
- `searchInFile()` / `searchInAllFiles()` - Search functionality
- `getFileStats()` - File statistics

## Usage Examples

### Basic Usage (Context Only)

```typescript
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';

function FileManager() {
  const { files, addFile, removeFile, isLoading } = useResourceAttachment();

  const handleAddFile = async () => {
    try {
      const attachment = await addFile('path/to/file.txt', 'text/plain');
      console.log('File added:', attachment.filename);
    } catch (error) {
      console.error('Failed to add file:', error);
    }
  };

  return (
    <div>
      {files.map(file => (
        <div key={file.contentId}>
          {file.filename} ({file.mimeType})
        </div>
      ))}
    </div>
  );
}
```

### Enhanced Usage (With Service Integration)

```typescript
import { useResourceAttachmentService } from '@/hooks/use-resource-attachment-service';

function AdvancedFileManager() {
  const {
    files,
    addAndProcessFile,
    searchInAllFiles,
    getFileStats
  } = useResourceAttachmentService();

  const handleAddFile = async () => {
    try {
      // This will use MCP tools for full processing
      const attachment = await addAndProcessFile(
        'https://example.com/document.pdf',
        'application/pdf',
        'my-document.pdf',
        { maxPreviewLines: 50, generateChunks: true }
      );
      console.log('File processed:', attachment);
    } catch (error) {
      console.error('Failed to process file:', error);
    }
  };

  const handleSearch = async () => {
    try {
      const results = await searchInAllFiles('important keyword');
      console.log('Search results:', results);
    } catch (error) {
      console.error('Search failed:', error);
    }
  };

  const stats = getFileStats();
  console.log('File statistics:', stats);

  return (
    <div>
      <h3>Files: {stats.totalFiles}</h3>
      <h3>Total Size: {stats.totalSize} bytes</h3>
      {/* File list and search UI */}
    </div>
  );
}
```

## MCP Integration Points

The system is designed with clear integration points for MCP tools:

### File System Operations

- **Local files**: Use MCP file system tools to read content, get metadata
- **Web URLs**: Use MCP web scraping tools to fetch and process content
- **Document processing**: Use specialized MCP tools for different file formats

### Content Processing

- **Text extraction**: From PDFs, documents, images
- **Metadata extraction**: File properties, document info
- **Content chunking**: For large documents and search optimization

### Search and Retrieval

- **Semantic search**: Using MCP-powered search tools
- **Content indexing**: For fast retrieval
- **Cross-file search**: Search across multiple attached files

## TODOs for MCP Integration

1. **File System Tools Integration**
   - Implement actual file reading using MCP file system tools
   - Add support for different file types (PDF, Word, etc.)
   - Implement proper error handling for file access

2. **Web Content Processing**
   - Integrate MCP web scraping tools for URL processing
   - Add support for different web content types
   - Implement caching for web content

3. **Search Implementation**
   - Integrate MCP search tools for semantic search
   - Implement content indexing
   - Add fuzzy search capabilities

4. **Content Chunking**
   - Implement intelligent document chunking
   - Add chunk overlap for better context
   - Optimize chunk size based on content type

## Benefits of This Architecture

1. **Separation of Concerns**: Clear separation between state management, business logic, and UI
2. **Testability**: Each layer can be tested independently
3. **Scalability**: Easy to extend with new file types and processing capabilities
4. **MCP Ready**: Designed with MCP integration points throughout
5. **Error Handling**: Comprehensive error handling and logging
6. **Type Safety**: Full TypeScript support with proper typing
7. **Performance**: Optimized with proper React patterns (useCallback, useMemo)

## File Structure

```
src/
├── context/
│   └── ResourceAttachmentContext.tsx     # State management
├── lib/
│   └── resource-attachment-service.ts    # Business logic
└── hooks/
    └── use-resource-attachment-service.ts # Combined interface
```

This architecture provides a solid foundation that can be easily extended as MCP tools become available, while maintaining clean code organization and excellent developer experience.
