import { safeInvoke } from './core';
import type { Session, Assistant } from '@/models/chat';
import type { AgentResponse, AgentSessionMetadata } from '@/models/agent-ipc';
import type { Page } from '@/lib/db/types';

interface SessionDto {
  id: string;
  name: string | null;
  createdAt: number;
  updatedAt: number;
  config: unknown; // AgentConfig
  activeThreadId: string | null;
}

function deserializeSession(dto: SessionDto): Session {
  let assistants: Assistant[] = [];

  if (dto.config) {
    try {
      const configObj =
        typeof dto.config === 'string' ? JSON.parse(dto.config) : dto.config;

      // Case 1: V2 style (direct Assistant object)
      if (configObj.systemPrompt && !configObj.assistants) {
        assistants = [configObj as Assistant];
      }
      // Case 2: Legacy/Group style (object with assistants IDs)
      else if (
        configObj.assistants &&
        Array.isArray(configObj.assistants) &&
        configObj.assistants.length > 0
      ) {
        assistants = configObj.assistants.map((id: string) => ({
          id,
          name: 'Loading...',
          systemPrompt: configObj.systemPrompt || '',
          createdAt: new Date(dto.createdAt),
          updatedAt: new Date(dto.updatedAt),
          deletionProtected: false,
        }));
      }
    } catch {
      // Ignore parse errors
    }
  }

  // Fallback if no assistants found
  if (assistants.length === 0) {
    assistants = [
      {
        id: 'unknown-assistant',
        name: 'Unknown Assistant',
        systemPrompt: 'You are a helpful assistant.',
        createdAt: new Date(dto.createdAt),
        updatedAt: new Date(dto.updatedAt),
        deletionProtected: false,
      },
    ];
  }

  return {
    id: dto.id,
    type: assistants.length > 1 ? 'group' : 'single',
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

export async function createSession(session: Session): Promise<Session> {
  // Use the first assistant as the primary agent config
  const assistant = session.assistants[0];
  if (!assistant) {
    throw new Error('Cannot create session without an assistant');
  }

  await safeInvoke<AgentSessionMetadata>('agent_create_session', {
    request: {
      sessionId: session.id,
      name: session.name,
      agentConfig: {
        ...assistant,
      },
      isEphemeral: false,
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
  await safeInvoke<AgentResponse>('agent_delete_session', { sessionId: id });
}

export async function deleteSessionOnly(id: string): Promise<void> {
  await safeInvoke<AgentResponse>('agent_delete_session_only', {
    sessionId: id,
  });
}

export async function toggleSessionBookmark(
  id: string,
  bookmarked: boolean,
): Promise<void> {
  await safeInvoke<void>('agent_toggle_session_bookmark', {
    sessionId: id,
    bookmarked,
  });
}

export async function markSessionViewed(
  id: string,
  viewedAt?: Date,
): Promise<void> {
  await safeInvoke<void>('agent_mark_session_viewed', {
    sessionId: id,
    ...(viewedAt ? { viewedAt: viewedAt.getTime() } : {}),
  });
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
    const assistant = session.assistants[0];
    if (!assistant) return;

    await safeInvoke<AgentResponse>('agent_update_session_config', {
      request: {
        sessionId: session.id,
        agentConfig: {
          ...assistant,
        },
      },
    });
  }
}
