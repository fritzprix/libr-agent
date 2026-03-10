/**
 * Regression tests for Assistant serialization / deserialization.
 *
 * Bug history:
 *  - AssistantContext.upsertAssistant() manually assembled `assistantToSave`
 *    and forgot to copy `description`, `avatar`, and `disabledSkills`.
 *    Any save (e.g. toggling an MCP server) silently dropped those fields.
 */

import { describe, it, expect } from 'vitest';
import { parseAssistant } from '../validation';

// ─── helpers ─────────────────────────────────────────────────────────────────

function makeDto(configOverrides: Record<string, unknown> = {}) {
  return {
    id: 'asst_test_001',
    name: 'Test Assistant',
    config: {
      systemPrompt: 'You are helpful.',
      description: 'Handles code review tasks.',
      avatar: 'bot-blue',
      disabledSkills: ['skill_a', 'skill_b'],
      mcpServerIds: ['srv_001', 'srv_002'],
      allowedBuiltInServiceAliases: ['workspace', 'planning'],
      deletionProtected: false,
      ...configOverrides,
    },
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_001_000,
  };
}

// ─── parseAssistant: field preservation ──────────────────────────────────────

describe('parseAssistant', () => {
  it('preserves description from config', () => {
    const assistant = parseAssistant(makeDto());
    expect(assistant.description).toBe('Handles code review tasks.');
  });

  it('preserves avatar from config', () => {
    const assistant = parseAssistant(makeDto());
    expect(assistant.avatar).toBe('bot-blue');
  });

  it('preserves disabledSkills from config', () => {
    const assistant = parseAssistant(makeDto());
    expect(assistant.disabledSkills).toEqual(['skill_a', 'skill_b']);
  });

  it('preserves mcpServerIds from config', () => {
    const assistant = parseAssistant(makeDto());
    expect(assistant.mcpServerIds).toEqual(['srv_001', 'srv_002']);
  });

  it('preserves allowedBuiltInServiceAliases from config', () => {
    const assistant = parseAssistant(makeDto());
    expect(assistant.allowedBuiltInServiceAliases).toEqual([
      'workspace',
      'planning',
    ]);
  });

  it('returns undefined description when not set', () => {
    const dto = makeDto();
    delete (dto.config as Record<string, unknown>).description;
    const assistant = parseAssistant(dto);
    expect(assistant.description).toBeUndefined();
  });

  it('handles stringified config (backend may return JSON string)', () => {
    const dto = {
      id: 'asst_str',
      name: 'Stringified',
      config: JSON.stringify({
        systemPrompt: 'Hello.',
        description: 'Stored as string.',
      }),
      createdAt: 0,
      updatedAt: 0,
    };
    const assistant = parseAssistant(dto);
    expect(assistant.description).toBe('Stored as string.');
  });

  it('handles invalid stringified config', () => {
    const dto = {
      id: 'asst_str_invalid',
      name: 'Invalid Stringified',
      config: '{ invalid json }',
      createdAt: 0,
      updatedAt: 0,
    };
    const assistant = parseAssistant(dto);
    expect(assistant.systemPrompt).toBe('You are a helpful assistant.');
  });

  it('handles missing config', () => {
    const dto = {
      id: 'asst_no_config',
      name: 'No Config',
      config: null,
      createdAt: 0,
      updatedAt: 0,
    };
    const assistant = parseAssistant(dto);
    expect(assistant.systemPrompt).toBe('You are a helpful assistant.');
  });
});

// ─── Round-trip: serialize → parse ───────────────────────────────────────────
// Simulates what AssistantContext.upsertAssistant() + backend does:
//   1. Build `assistantToSave` object
//   2. serializeAssistant() → config JSON string
//   3. Backend stores & returns DTO
//   4. parseAssistant() hydrates the model
//
// The bug: step 1 forgot to copy description/avatar/disabledSkills,
// so they never reached step 2.

describe('AssistantContext save round-trip', () => {
  /**
   * Simulate the (now-fixed) assistantToSave construction in AssistantContext.
   * All optional fields must be forwarded explicitly.
   */
  function buildAssistantToSave(editing: ReturnType<typeof parseAssistant>) {
    return {
      id: editing.id,
      name: editing.name,
      description: editing.description, // ← was missing before fix
      avatar: editing.avatar, // ← was missing before fix
      systemPrompt: editing.systemPrompt,
      mcpServerIds: editing.mcpServerIds,
      deletionProtected: editing.deletionProtected ?? false,
      localServices: editing.localServices ?? [],
      disabledSkills: editing.disabledSkills, // ← was missing before fix
      allowedBuiltInServiceAliases: editing.allowedBuiltInServiceAliases,
      createdAt: editing.createdAt,
      updatedAt: new Date(),
    };
  }

  function simulateBackendRoundTrip(
    saved: ReturnType<typeof buildAssistantToSave>,
  ) {
    // serializeAssistant spreads everything except id/name into config
    const { id, name, createdAt, updatedAt, ...configRest } = saved;
    const configJson = JSON.stringify(configRest);

    // backend returns DTO with config as parsed object
    return parseAssistant({
      id,
      name,
      config: JSON.parse(configJson),
      createdAt: createdAt.getTime(),
      updatedAt: updatedAt.getTime(),
    });
  }

  it('description survives a save round-trip', () => {
    const original = parseAssistant(makeDto());
    const saved = buildAssistantToSave(original);
    const reloaded = simulateBackendRoundTrip(saved);
    expect(reloaded.description).toBe(original.description);
  });

  it('avatar survives a save round-trip', () => {
    const original = parseAssistant(makeDto());
    const saved = buildAssistantToSave(original);
    const reloaded = simulateBackendRoundTrip(saved);
    expect(reloaded.avatar).toBe(original.avatar);
  });

  it('disabledSkills survive a save round-trip', () => {
    const original = parseAssistant(makeDto());
    const saved = buildAssistantToSave(original);
    const reloaded = simulateBackendRoundTrip(saved);
    expect(reloaded.disabledSkills).toEqual(original.disabledSkills);
  });

  it('mcpServerIds survive a save round-trip', () => {
    const original = parseAssistant(makeDto());
    const saved = buildAssistantToSave(original);
    const reloaded = simulateBackendRoundTrip(saved);
    expect(reloaded.mcpServerIds).toEqual(original.mcpServerIds);
  });

  it('adding an MCP server does not wipe description', () => {
    const original = parseAssistant(makeDto());

    // Simulate: user opens editor, adds a new MCP server, saves
    const editingWithNewServer = {
      ...original,
      mcpServerIds: [...(original.mcpServerIds ?? []), 'srv_new'],
    };

    const saved = buildAssistantToSave(editingWithNewServer);
    const reloaded = simulateBackendRoundTrip(saved);

    expect(reloaded.description).toBe(original.description);
    expect(reloaded.mcpServerIds).toContain('srv_new');
  });
});
