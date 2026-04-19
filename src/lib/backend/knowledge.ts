import { safeInvoke } from './core';

export interface KnowledgeChunkListItem {
  id: number;
  assistantId: string;
  preview: string;
  tags: string[];
  source?: string | null;
  createdAt: number;
}

export interface GlobalKnowledgeListResponse {
  items: KnowledgeChunkListItem[];
  assistants: string[];
  nextCursor?: KnowledgeListCursor | null;
}

export interface KnowledgeListCursor {
  createdAt: number;
  id: number;
}

export interface KnowledgeGraphEntity {
  id: number;
  assistantId: string;
  name: string;
  entityType?: string | null;
  description?: string | null;
  isPrimary: boolean;
}

export interface KnowledgeGraphRelationship {
  id: number;
  assistantId: string;
  sourceEntityId: number;
  targetEntityId: number;
  relationType: string;
  weight: number;
}

export interface KnowledgeChunkDetail {
  id: number;
  assistantId: string;
  content: string;
  tags: string[];
  source?: string | null;
  createdAt: number;
  primaryEntityIds: number[];
  entities: KnowledgeGraphEntity[];
  relationships: KnowledgeGraphRelationship[];
}

export interface DeleteGlobalKnowledgeResponse {
  deletedChunkId: number;
  orphanEntityCount: number;
  orphanRelationshipCount: number;
}

export interface ListGlobalKnowledgeRequest {
  query?: string;
  assistantId?: string;
  cursor?: KnowledgeListCursor;
  limit?: number;
}

export async function listGlobalKnowledge(
  request: ListGlobalKnowledgeRequest = {},
): Promise<GlobalKnowledgeListResponse> {
  return safeInvoke<GlobalKnowledgeListResponse>('list_global_knowledge', {
    request,
  });
}

export async function getGlobalKnowledgeDetail(
  id: number,
): Promise<KnowledgeChunkDetail> {
  return safeInvoke<KnowledgeChunkDetail>('get_global_knowledge_detail', {
    id,
  });
}

export async function deleteGlobalKnowledge(
  id: number,
): Promise<DeleteGlobalKnowledgeResponse> {
  return safeInvoke<DeleteGlobalKnowledgeResponse>('delete_global_knowledge', {
    id,
  });
}
