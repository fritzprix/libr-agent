/**
 * Content Store Module - Main Entry Point
 *
 * This file maintains the default export for MCP Worker dynamic loading compatibility.
 * The structure supports Vite bundling and Web Worker environments.
 */

// Import the main server implementation
import fileStoreServer from './server';

// Export submodules for direct access if needed
export { logger } from './logger';
export { parseRichFile, ParserFactory, ParserError } from './parser';
export { TextChunker } from './chunker';
export { contentSearchEngine } from './search';

// Export server types
export type {
  CreateStoreInput,
  CreateStoreOutput,
  AddContentInput,
  AddContentOutput,
  ListContentInput,
  ReadContentInput,
  SimilaritySearchInput,
  ContentSummary,
  ContentStoreServer,
} from './server';

export type {
  ParseResult,
  StoreInfo,
  ContentInfo,
  SearchOptions,
  ContentSearchResult,
  CreateStoreParams,
  ListStoresParams,
  AddFileParams,
  SearchParams,
  ReadContentParams,
  DeleteContentParams,
  DeleteStoreParams,
  ListContentParams,
  FileStoreError,
  StoreNotFoundError,
  ContentNotFoundError,
  InvalidRangeError,
  MAX_FILE_SIZE,
  MAX_CONTENT_LENGTH,
} from './types';

// Default export for MCP Worker dynamic loading
// This maintains compatibility with: import('./modules/content-store')
export default fileStoreServer;
