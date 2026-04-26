import type { AgentSession } from '@/models/agent';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import type { Assistant } from '@/models/chat';

function readStringField(
  record: Record<string, unknown>,
  ...keys: string[]
): string | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'string' && value.length > 0) {
      return value;
    }
  }
  return undefined;
}

function readNumberField(
  record: Record<string, unknown>,
  ...keys: string[]
): number | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
  }
  return undefined;
}

function readBooleanField(
  record: Record<string, unknown>,
  ...keys: string[]
): boolean | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'boolean') {
      return value;
    }
  }
  return undefined;
}

function readStringArrayField(
  record: Record<string, unknown>,
  ...keys: string[]
): string[] | undefined {
  for (const key of keys) {
    const value = record[key];
    if (
      Array.isArray(value) &&
      value.every((item): item is string => typeof item === 'string')
    ) {
      return value;
    }
  }
  return undefined;
}

interface ParsedAgentConfigMetadata {
  assistant?: Assistant;
  parentSessionId?: string;
  lineageId?: string;
  depth?: number;
  orgId?: string;
  orgName?: string;
  orgRootSessionId?: string;
}

function buildAssistantFromAgentConfig(
  record: Record<string, unknown>,
  createdAt: number,
  updatedAt?: number,
): Assistant | undefined {
  const id = readStringField(record, 'id', 'assistantId', 'assistant_id');
  const name = readStringField(record, 'name');
  const systemPrompt = readStringField(record, 'systemPrompt', 'system_prompt');

  if (!id || !name || !systemPrompt) {
    return undefined;
  }

  return {
    id,
    name,
    description: readStringField(record, 'description'),
    avatar: readStringField(record, 'avatar'),
    systemPrompt,
    mcpServerIds: readStringArrayField(
      record,
      'mcpServerIds',
      'mcp_server_ids',
    ),
    localServices: readStringArrayField(
      record,
      'localServices',
      'local_services',
    ),
    disabledSkills: readStringArrayField(
      record,
      'disabledSkills',
      'disabled_skills',
    ),
    allowedBuiltInServiceAliases: readStringArrayField(
      record,
      'allowedBuiltInServiceAliases',
      'allowed_built_in_service_aliases',
    ),
    deletionProtected:
      readBooleanField(record, 'deletionProtected', 'deletion_protected') ??
      false,
    createdAt: new Date(createdAt),
    updatedAt: new Date(updatedAt ?? createdAt),
  };
}

export function parseAgentConfigMetadata(
  agentConfig: string | undefined,
  createdAt: number,
  updatedAt?: number,
): ParsedAgentConfigMetadata {
  if (!agentConfig) {
    return {};
  }

  try {
    const parsed: unknown = JSON.parse(agentConfig);
    if (typeof parsed !== 'object' || parsed === null) {
      return {};
    }

    const record = parsed as Record<string, unknown>;

    return {
      assistant: buildAssistantFromAgentConfig(record, createdAt, updatedAt),
      parentSessionId: readStringField(
        record,
        'parentSessionId',
        'parent_session_id',
      ),
      lineageId: readStringField(record, 'lineageId', 'lineage_id'),
      depth: readNumberField(record, 'depth'),
      orgId: readStringField(record, 'orgId', 'org_id'),
      orgName: readStringField(record, 'orgName', 'org_name'),
      orgRootSessionId: readStringField(
        record,
        'orgRootSessionId',
        'org_root_session_id',
      ),
    };
  } catch {
    return {};
  }
}

export function mapSessionMetadataToAgentSession(
  metadata: AgentSessionMetadata,
  pendingApprovalCount = 0,
): AgentSession {
  const parsedConfig = parseAgentConfigMetadata(
    metadata.agentConfig,
    metadata.createdAt,
    metadata.updatedAt,
  );

  const parentSessionId =
    metadata.parentSessionId ?? parsedConfig.parentSessionId;
  const lineageId =
    metadata.lineageId ??
    parsedConfig.lineageId ??
    parentSessionId ??
    metadata.id;
  const depth =
    metadata.depth ?? parsedConfig.depth ?? (parentSessionId ? 1 : 0);

  return {
    id: metadata.id,
    name: metadata.name,
    status: metadata.status,
    model: metadata.model,
    provider: metadata.provider,
    assistant: parsedConfig.assistant,
    parentSessionId,
    lineageId,
    depth,
    orgId: metadata.orgId ?? parsedConfig.orgId,
    orgName: metadata.orgName ?? parsedConfig.orgName,
    orgRootSessionId:
      metadata.orgRootSessionId ?? parsedConfig.orgRootSessionId,
    createdAt: new Date(metadata.createdAt),
    updatedAt: metadata.updatedAt ? new Date(metadata.updatedAt) : undefined,
    lastViewedAt: metadata.lastViewedAt
      ? new Date(metadata.lastViewedAt)
      : undefined,
    lastMessageAt: metadata.lastMessageAt
      ? new Date(metadata.lastMessageAt)
      : undefined,
    lastAttentionAt: metadata.lastAttentionAt
      ? new Date(metadata.lastAttentionAt)
      : undefined,
    lastAttentionReason: metadata.lastAttentionReason,
    isBookmarked: metadata.isBookmarked ?? false,
    yoloMode: metadata.yoloMode ?? false,
    pendingApprovalCount,
  };
}

export function sortSessionsByLatestActivity(
  sessions: AgentSession[],
): AgentSession[] {
  return sessions.slice().sort((a, b) => {
    const timeA = a.updatedAt?.getTime() || a.createdAt.getTime();
    const timeB = b.updatedAt?.getTime() || b.createdAt.getTime();
    return timeB - timeA;
  });
}
