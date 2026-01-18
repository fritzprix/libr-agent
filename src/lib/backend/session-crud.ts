import { safeInvoke } from './core';
import type { Session, Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { safeParseAgentConfig } from '@/lib/schemas/agent-config';

interface SessionDto {
  id: string;
  name: string | null;
  createdAt: number;
  updatedAt: number;
  config: unknown; // AgentConfig
  activeThreadId: string | null;
}

interface AgentConfig {
  systemPrompt?: string;
  mcpServers?: string[];
  assistants?: string[]; // Assistant IDs
}

function deserializeSession(dto: SessionDto): Session {
  // We need to reconstruct Session object.
  // Requires resolving assistants if possible, or keeping them as IDs?
  // Frontend Session has `assistants: Assistant[]`.
  // Backend stores IDs in config.
  // Note: List/Get often acts as metadata retrieval. Resolving assistants might require extra calls.
  // For basic listing, we might return empty assistants or use a separate "HydratedSession" type?
  // But strictly adhering to `Session` interface requires `assistants` array.

  // Simulation: We assume the caller might need to hydrate assistants separately
  // or we return minimal Assistant objects (just with IDs).
  let config: AgentConfig = {};
  if (typeof dto.config === 'string') {
    config = safeParseAgentConfig(dto.config);
  } else if (dto.config && typeof dto.config === 'object') {
    const result = safeParseAgentConfig(JSON.stringify(dto.config));
    config = result || {};
  }

  const assistantIds = config.assistants || [];

  // Minimal assistants stub
  const assistants: Assistant[] = assistantIds.map((id) => ({
    id,
    name: 'Loading...',
    systemPrompt: '',
    createdAt: new Date(),
    updatedAt: new Date(),
    deletionProtected: false,
  }));

  return {
    id: dto.id,
    type: helpers.determineType(assistantIds), // helper needed
    assistants,
    name: dto.name || undefined,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
    sessionThread: {
      id: dto.id,
      sessionId: dto.id,
      createdAt: new Date(dto.createdAt),
    },
  };
}

const helpers = {
  determineType: (ids: string[]): 'single' | 'group' =>
    ids.length > 1 ? 'group' : 'single',
};

export async function createSession(session: Session): Promise<Session> {
  // This calls agent_create_session which expects AgentConfig
  // We map frontend Session to CreateAgentSessionRequest
  const assistantIds = session.assistants.map((a) => a.id!);

  // We need to invoke 'agent_create_session'
  // But `agent_commands.rs` implementation takes `CreateAgentSessionRequest`
  /*
    pub struct CreateAgentSessionRequest {
        pub session_id: String,
        pub name: Option<String>,
        pub agent_config: AgentConfig,
    }
    */

  await safeInvoke('agent_create_session', {
    request: {
      sessionId: session.id,
      name: session.name,
      agentConfig: {
        systemPrompt: session.assistants[0]?.systemPrompt, // Legacy simplifiction?
        mcpServers: [], // TODO: extract from assistants
        env: {},
        assistants: assistantIds,
      },
    },
  });

  return session;
}

export async function getSession(id: string): Promise<Session | undefined> {
  const dto = await safeInvoke<SessionDto | null>('agent_get_session', {
    sessionId: id,
  });
  return dto ? deserializeSession(dto) : undefined;
}

export async function listSessions(): Promise<Session[]> {
  const dtos = await safeInvoke<SessionDto[]>('agent_get_all_sessions');
  // Sort desc by updated
  return dtos
    .map(deserializeSession)
    .sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());
}

export async function deleteSession(id: string): Promise<void> {
  await safeInvoke('agent_delete_session', { sessionId: id });
}

export async function getSessionsPage(
  page: number,
  pageSize: number,
): Promise<Page<Session>> {
  const all = await listSessions();
  const totalItems = all.length;
  if (pageSize === -1) {
    return {
      items: all,
      page: 1,
      pageSize: totalItems,
      totalItems,
      totalPages: 1,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  const totalPages = Math.ceil(totalItems / pageSize) || 1;
  const start = (page - 1) * pageSize;
  const end = start + pageSize;
  const items = all.slice(start, end);

  return {
    items,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
}

export async function upsertSession(session: Session): Promise<void> {
  // Check exist
  const exists = await getSession(session.id);
  if (!exists) {
    await createSession(session);
  } else {
    // Update?? agent_update_session_config
    // This splits update logic.
    // For metadata (name), maybe just ignore?
    // Or implement update session command.
    // `agent_update_session_config` exists.
    const assistantIds = session.assistants.map((a) => a.id!);
    await safeInvoke('agent_update_session_config', {
      request: {
        sessionId: session.id,
        agentConfig: {
          systemPrompt: session.assistants[0]?.systemPrompt,
          mcpServers: [],
          env: {},
          assistants: assistantIds,
        },
      },
    });
  }
}
