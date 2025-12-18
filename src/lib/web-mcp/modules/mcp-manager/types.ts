import type { MCPServerEntity } from '@/models/chat';

// Input type interfaces for type safety
export interface ListServersInput {
  page?: number;
  pageSize?: number;
  filterByAssistant?: boolean;
  includeInactive?: boolean;
}

export interface SearchServersInput {
  query: string;
  page?: number;
  pageSize?: number;
  searchMode?: 'bm25' | 'simple';
  byNameOnly?: boolean;
  includeInactive?: boolean;
  weights?: {
    nameWeight?: number;
    descWeight?: number;
  };
}

export interface CreateServerInput {
  name: string;
  description?: string;
  transport: unknown;
  tags?: string[];
}

export interface ConnectInput {
  serverId?: string;
  serverName?: string;
  scope?: 'assistant' | 'global';
  autoStart?: boolean;
}

export interface DisconnectInput {
  serverId?: string;
  serverName?: string;
  scope?: 'assistant' | 'global';
}

// Response types for structured content
export interface ListServersOutput {
  items: MCPServerEntity[];
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  hasNextPage: boolean;
  hasPreviousPage: boolean;
}

export interface SearchServersOutput extends ListServersOutput {
  query: string;
  mode?: string;
}

export interface CreateServerOutput {
  server: MCPServerEntity;
  message: string;
}

export interface ConnectServerOutput {
  success: boolean;
  server: MCPServerEntity;
  scope: 'assistant' | 'global';
  message: string;
  assistantId?: string;
}
