import { describe, expect, it } from 'vitest';

import {
  coalesceExecutionModeFlags,
  mapSessionMetadataToAgentSession,
} from '../session-metadata';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import type { Assistant } from '@/models/chat';

describe('mapSessionMetadataToAgentSession', () => {
  const metadata: AgentSessionMetadata = {
    id: 'session-1',
    name: 'Test Session',
    status: 'idle',
    model: 'gpt-4.1',
    provider: 'openai',
    assistantId: 'assistant-1',
    parentSessionId: 'parent-1',
    lineageId: 'lineage-1',
    depth: 2,
    orgId: 'org-1',
    orgName: 'Org One',
    orgRootSessionId: 'root-1',
    createdAt: 1_000,
    updatedAt: 2_000,
    yoloMode: false,
  };

  const assistant: Assistant = {
    id: 'assistant-1',
    name: 'Planner',
    description: 'Keeps plans tidy',
    systemPrompt: 'Plan carefully.',
    createdAt: new Date(1_000),
    updatedAt: new Date(2_000),
    deletionProtected: false,
  };

  it('maps session columns and attaches the resolved assistant', () => {
    const session = mapSessionMetadataToAgentSession(metadata, 3, assistant);

    expect(session.id).toBe('session-1');
    expect(session.assistant).toEqual(assistant);
    expect(session.parentSessionId).toBe('parent-1');
    expect(session.lineageId).toBe('lineage-1');
    expect(session.depth).toBe(2);
    expect(session.orgId).toBe('org-1');
    expect(session.orgName).toBe('Org One');
    expect(session.orgRootSessionId).toBe('root-1');
    expect(session.pendingApprovalCount).toBe(3);
  });

  it('derives lineage defaults when optional metadata is missing', () => {
    const session = mapSessionMetadataToAgentSession({
      ...metadata,
      parentSessionId: undefined,
      lineageId: undefined,
      depth: undefined,
    });

    expect(session.lineageId).toBe('session-1');
    expect(session.depth).toBe(0);
  });
});

describe('coalesceExecutionModeFlags', () => {
  it('maps legacy yolo-only sessions to yolo mode', () => {
    expect(coalesceExecutionModeFlags(true, undefined)).toEqual({
      executionMode: 'yolo',
      yoloMode: true,
      unsafeMode: false,
    });
  });

  it('prefers unsafe mode when legacy flags are both enabled', () => {
    expect(coalesceExecutionModeFlags(true, true)).toEqual({
      executionMode: 'unsafe',
      yoloMode: false,
      unsafeMode: true,
    });
  });
});
