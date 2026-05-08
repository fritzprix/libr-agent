import { enforceRuntimeBuiltinAliases } from '@/lib/assistant/runtime-builtins';
import {
  mapSessionMetadataToAgentSession,
  sortSessionsByLatestActivity,
} from '@/lib/session-metadata';
import type { AgentSession } from '@/models/agent';
import type { Assistant } from '@/models/chat';
import type {
  AgentConfig,
  AgentSessionListResponse,
  AgentSessionMetadata,
  CreateAgentSessionRequest,
} from '@/models/agent-ipc';

interface SessionRuntimeLimits {
  defaultSessionMaxDepth: number;
  defaultSessionMaxFanout: number;
}

function isAgentSessionListResponse(
  response: unknown,
): response is AgentSessionListResponse {
  return (
    typeof response === 'object' &&
    response !== null &&
    'items' in response &&
    Array.isArray(response.items)
  );
}

export function normalizeSessionListResponse(
  response: AgentSessionListResponse | AgentSessionMetadata[] | unknown,
): AgentSessionListResponse {
  if (Array.isArray(response)) {
    return {
      items: response,
      nextCursor: undefined,
    };
  }

  if (isAgentSessionListResponse(response)) {
    return {
      items: response.items,
      nextCursor: response.nextCursor,
    };
  }

  return {
    items: [],
    nextCursor: undefined,
  };
}

export function normalizeAttentionSessions(
  attentionSessions: AgentSessionMetadata[] | unknown,
): AgentSessionMetadata[] {
  return Array.isArray(attentionSessions) ? attentionSessions : [];
}

export function mapSessionMetadataList(
  sessionMetadataList: AgentSessionMetadata[],
  pendingApprovalCounts: Map<string, number>,
): AgentSession[] {
  return sortSessionsByLatestActivity(
    sessionMetadataList.map((sessionMetadata) =>
      mapSessionMetadataToAgentSession(
        sessionMetadata,
        pendingApprovalCounts.get(sessionMetadata.id) ?? 0,
      ),
    ),
  );
}

export function buildAgentConfig(
  assistant: Assistant,
  runtimeLimits: SessionRuntimeLimits,
): AgentConfig {
  return {
    id: assistant.id,
    name: assistant.name,
    description: assistant.description,
    systemPrompt: assistant.systemPrompt,
    mcpServerIds: assistant.mcpServerIds ?? [],
    localServices: assistant.localServices ?? [],
    allowedBuiltInServiceAliases: enforceRuntimeBuiltinAliases(
      assistant.allowedBuiltInServiceAliases,
    ),
    temperature: 1.0,
    ...(runtimeLimits.defaultSessionMaxDepth > 0
      ? { maxDepth: runtimeLimits.defaultSessionMaxDepth }
      : {}),
    ...(runtimeLimits.defaultSessionMaxFanout > 0
      ? { maxFanout: runtimeLimits.defaultSessionMaxFanout }
      : {}),
  };
}

export function buildCreateAgentSessionRequest(args: {
  assistant: Assistant;
  name?: string;
  modelId: string;
  provider: string;
  runtimeLimits: SessionRuntimeLimits;
  sessionId: string;
}): CreateAgentSessionRequest {
  const { assistant, modelId, name, provider, runtimeLimits, sessionId } = args;

  return {
    sessionId,
    name: name || `Conversation with ${assistant.name}`,
    model: modelId,
    provider,
    agentConfig: buildAgentConfig(assistant, runtimeLimits),
  };
}

export function mapCreatedSession(
  metadata: AgentSessionMetadata,
  assistant: Assistant,
): AgentSession {
  return {
    ...mapSessionMetadataToAgentSession(metadata),
    assistant,
    pendingApprovalCount: 0,
  };
}
