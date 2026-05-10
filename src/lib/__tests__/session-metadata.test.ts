import { describe, expect, it } from 'vitest';

import {
  coalesceExecutionModeFlags,
  parseAgentConfigMetadata,
} from '../session-metadata';

describe('parseAgentConfigMetadata', () => {
  const agentConfig = JSON.stringify({
    id: 'assistant-1',
    name: 'Planner',
    systemPrompt: 'Plan carefully.',
    description: 'Keeps plans tidy',
    mcpServerIds: ['mcp-1'],
    localServices: ['workspace'],
    disabledSkills: ['skill-a'],
    allowedBuiltInServiceAliases: ['planning'],
    parentSessionId: 'parent-1',
    lineageId: 'lineage-1',
    depth: 2,
    orgId: 'org-1',
    orgName: 'Org One',
    orgRootSessionId: 'root-1',
  });

  it('preserves timestamp-derived assistant dates across repeated parses of the same config', () => {
    const first = parseAgentConfigMetadata(agentConfig, 1000, 2000);
    const second = parseAgentConfigMetadata(agentConfig, 3000, 4000);

    expect(first.assistant?.createdAt.getTime()).toBe(1000);
    expect(first.assistant?.updatedAt.getTime()).toBe(2000);
    expect(second.assistant?.createdAt.getTime()).toBe(3000);
    expect(second.assistant?.updatedAt.getTime()).toBe(4000);
  });

  it('returns fresh array instances even when the parsed config is cached', () => {
    const first = parseAgentConfigMetadata(agentConfig, 1000, 2000);
    const second = parseAgentConfigMetadata(agentConfig, 1000, 2000);

    expect(first.assistant?.allowedBuiltInServiceAliases).toEqual(['planning']);
    expect(second.assistant?.allowedBuiltInServiceAliases).toEqual(['planning']);
    expect(first.assistant?.allowedBuiltInServiceAliases).not.toBe(
      second.assistant?.allowedBuiltInServiceAliases,
    );

    first.assistant?.allowedBuiltInServiceAliases?.push('knowledge');
    first.assistant?.mcpServerIds?.push('mcp-2');

    expect(second.assistant?.allowedBuiltInServiceAliases).toEqual([
      'planning',
    ]);
    expect(second.assistant?.mcpServerIds).toEqual(['mcp-1']);
  });

  it('returns metadata fields from cached parsed config without changing behavior', () => {
    const parsed = parseAgentConfigMetadata(agentConfig, 1000, 2000);

    expect(parsed.parentSessionId).toBe('parent-1');
    expect(parsed.lineageId).toBe('lineage-1');
    expect(parsed.depth).toBe(2);
    expect(parsed.orgId).toBe('org-1');
    expect(parsed.orgName).toBe('Org One');
    expect(parsed.orgRootSessionId).toBe('root-1');
    expect(parsed.assistant?.name).toBe('Planner');
    expect(parsed.assistant?.description).toBe('Keeps plans tidy');
    expect(parsed.assistant?.localServices).toEqual(['workspace']);
    expect(parsed.assistant?.disabledSkills).toEqual(['skill-a']);
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
