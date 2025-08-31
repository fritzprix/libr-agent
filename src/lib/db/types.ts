import type { Assistant, Group, Message, Session } from '@/models/chat';

export interface DatabaseObject<T = unknown> {
  key: string;
  value: T;
  createdAt?: Date;
  updatedAt?: Date;
}

export interface FileStore {
  id: string;
  name: string;
  description?: string;
  sessionId?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface FileContent {
  id: string;
  storeId: string;
  filename: string;
  mimeType: string;
  size: number;
  uploadedAt: Date;
  content: string;
  lineCount: number;
  summary: string;
  contentHash?: string;
}

export interface FileChunk {
  id: string;
  contentId: string;
  chunkIndex: number;
  text: string;
  startLine: number;
  endLine: number;
  embedding?: number[];
}

export interface Page<T> {
  items: T[];
  page: number;
  pageSize: number;
  totalItems: number;
  hasNextPage: boolean;
  hasPreviousPage: boolean;
}

export interface CRUD<T, U = T> {
  upsert: (object: T) => Promise<void>;
  upsertMany: (objects: T[]) => Promise<void>;
  read: (key: string) => Promise<U | undefined>;
  delete: (key: string) => Promise<void>;
  getPage: (page: number, pageSize: number) => Promise<Page<U>>;
  count: () => Promise<number>;
}

export interface FileContentCRUD extends CRUD<FileContent> {
  findByHashAndStore: (
    contentHash: string,
    storeId: string,
  ) => Promise<FileContent | undefined>;
}

export interface DatabaseService {
  assistants: CRUD<Assistant>;
  objects: CRUD<DatabaseObject<unknown>, DatabaseObject<unknown>>;
  sessions: CRUD<Session>;
  messages: CRUD<Message>;
  groups: CRUD<Group>;
  fileStores: CRUD<FileStore>;
  fileContents: FileContentCRUD;
  fileChunks: CRUD<FileChunk>;
}
