import type { Assistant, Group, Message, Session } from '@/models/chat';
import type { Playbook } from '@/types/playbook';

/**
 * Represents a generic key-value object in the database.
 * This can be used to store arbitrary data that doesn't have a specific schema.
 * @template T The type of the value being stored.
 */
export interface DatabaseObject<T = unknown> {
  /** The unique key for the object. */
  key: string;
  /** The data payload of the object. */
  value: T;
  /** The date and time when the object was created. */
  createdAt?: Date;
  /** The date and time when the object was last updated. */
  updatedAt?: Date;
}

/**
 * Represents a collection of files, similar to a folder.
 * It is typically associated with a specific chat session.
 */
export interface FileStore {
  /** The unique identifier for the file store. */
  id: string;
  /** The name of the file store. */
  name: string;
  /** An optional description for the file store. */
  description?: string;
  /** The ID of the session this file store belongs to. */
  sessionId?: string;
  /** The date and time when the file store was created. */
  createdAt: Date;
  /** The date and time when the file store was last updated. */
  updatedAt: Date;
}

/**
 * Represents the metadata and content of a single file.
 */
export interface FileContent {
  /** The unique identifier for the file content. */
  id: string;
  /** The ID of the `FileStore` this file belongs to. */
  storeId: string;
  /** The name of the file. */
  filename: string;
  /** The MIME type of the file (e.g., 'text/plain'). */
  mimeType: string;
  /** The size of the file in bytes. */
  size: number;
  /** The date and time when the file was uploaded. */
  uploadedAt: Date;
  /** The full content of the file as a string. */
  content: string;
  /** The total number of lines in the file. */
  lineCount: number;
  /** A summary of the file's content. */
  summary: string;
  /** An optional hash of the file's content for deduplication. */
  contentHash?: string;
}

/**
 * Represents a smaller chunk of a `FileContent` object.
 * Large files are broken down into chunks for easier processing and embedding.
 */
export interface FileChunk {
  /** The unique identifier for the file chunk. */
  id: string;
  /** The ID of the `FileContent` this chunk belongs to. */
  contentId: string;
  /** The sequential index of this chunk within the file. */
  chunkIndex: number;
  /** The text content of this chunk. */
  text: string;
  /** The starting line number of this chunk in the original file. */
  startLine: number;
  /** The ending line number of this chunk in the original file. */
  endLine: number;
  /** An optional vector embedding of the chunk's text content. */
  embedding?: number[];
}

/**
 * Represents a paginated slice of data.
 * @template T The type of items on the page.
 */
export interface Page<T> {
  /** The array of items for the current page. */
  items: T[];
  /** The current page number. */
  page: number;
  /** The number of items per page. */
  pageSize: number;
  /** The total number of items across all pages. */
  totalItems: number;
  /** A boolean indicating if there is a next page. */
  hasNextPage: boolean;
  /** A boolean indicating if there is a previous page. */
  hasPreviousPage: boolean;
}

/**
 * Defines a standard interface for Create, Read, Update, Delete (CRUD) operations.
 * @template T The type of the object for write operations (create/update).
 * @template U The type of the object for read operations (defaults to T).
 */
export interface CRUD<T, U = T> {
  /**
   * Creates a new object or updates an existing one.
   * @param object The object to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  upsert: (object: T) => Promise<void>;
  /**
   * Creates or updates multiple objects in a single operation.
   * @param objects An array of objects to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  upsertMany: (objects: T[]) => Promise<void>;
  /**
   * Reads an object from the database by its key.
   * @param key The unique key of the object to read.
   * @returns A promise that resolves to the object, or undefined if not found.
   */
  read: (key: string) => Promise<U | undefined>;
  /**
   * Deletes an object from the database by its key.
   * @param key The unique key of the object to delete.
   * @returns A promise that resolves when the operation is complete.
   */
  delete: (key: string) => Promise<void>;
  /**
   * Retrieves a paginated list of objects.
   * @param page The page number to retrieve.
   * @param pageSize The number of items per page.
   * @returns A promise that resolves to a `Page` of objects.
   */
  getPage: (page: number, pageSize: number) => Promise<Page<U>>;
  /**
   * Counts the total number of objects in the table.
   * @returns A promise that resolves to the total count.
   */
  count: () => Promise<number>;
}

/**
 * Extends the basic CRUD interface with additional methods specific to `FileContent`.
 */
export interface FileContentCRUD extends CRUD<FileContent> {
  /**
   * Finds a file content entry by its content hash and store ID.
   * This is useful for preventing duplicate file uploads.
   *
   * @param contentHash The hash of the file content.
   * @param storeId The ID of the store to search within.
   * @returns A promise that resolves to the `FileContent` object, or undefined if not found.
   */
  findByHashAndStore: (
    contentHash: string,
    storeId: string,
  ) => Promise<FileContent | undefined>;
}

/**
 * Defines the structure of the main database service.
 * It aggregates all the individual CRUD interfaces for each data model
 * into a single, cohesive service interface.
 */
export interface DatabaseService {
  /** CRUD operations for `Assistant` objects. */
  assistants: CRUD<Assistant>;
  /** Generic CRUD operations for `DatabaseObject`s. */
  objects: CRUD<DatabaseObject<unknown>, DatabaseObject<unknown>>;
  /** CRUD operations for `Session` objects. */
  sessions: CRUD<Session>;
  /** CRUD operations for `Message` objects. */
  messages: CRUD<Message>;
  /** CRUD operations for `Group` objects. */
  groups: CRUD<Group>;
  /** CRUD operations for `FileStore` objects. */
  fileStores: CRUD<FileStore>;
  /** CRUD operations for `FileContent` objects. */
  fileContents: FileContentCRUD;
  /** CRUD operations for `FileChunk` objects. */
  fileChunks: CRUD<FileChunk>;
  /** CRUD operations for persisted Playbook objects. */
  playbooks?: CRUD<Playbook & { id: string }>;
}
