import { act, renderHook } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import type { PluginCommandInfo, SkillMetadata } from '@/types/skills';
import type { MCPTool } from '@/lib/mcp';
import { useInputToken } from '../useInputToken';

const mockSkills: SkillMetadata[] = [
  {
    name: 'alpha',
    description: 'Alpha skill',
    path: '/skills/alpha',
    origin: 'user',
    source: 'global',
  },
  {
    name: 'beta',
    description: 'Beta skill',
    path: '/skills/beta',
    origin: 'user',
    source: 'global',
  },
];

const mockTools: MCPTool[] = [
  {
    name: 'search',
    description: 'Search tool',
    inputSchema: { type: 'object' },
  },
  {
    name: 'browser',
    description: 'Browser tool',
    inputSchema: { type: 'object' },
  },
];

const mockPluginCommands: PluginCommandInfo[] = [
  {
    plugin: 'alpha',
    command: 'hello',
    invocation: '/alpha:hello',
    bareInvocation: '/hello',
    description: 'say hi',
    argumentHint: '<name>',
    scope: 'global',
    sourceId: 'plugin-1',
  },
  {
    plugin: 'beta',
    command: 'deploy',
    invocation: '/beta:deploy',
    description: '',
    scope: 'session',
    sourceId: 'plugin-2',
  },
];

describe('useInputToken', () => {
  it('should identify a command when typing starts with /', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('/clear', 6);
    });

    expect(result.current.stage).toEqual({
      kind: 'typing-command',
      query: 'clear',
      anchorIndex: 0,
    });
    expect(result.current.commandResults).toHaveLength(1);
    expect(result.current.commandResults[0].id).toBe('/clear');
  });

  it('should identify a command when typing / after a space', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('hello /clear', 12);
    });

    expect(result.current.stage).toEqual({
      kind: 'typing-command',
      query: 'clear',
      anchorIndex: 6,
    });
    expect(result.current.commandResults).toHaveLength(1);
  });

  it('should identify /reload command when typing /reload', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('/reload', 7);
    });

    expect(result.current.stage).toEqual({
      kind: 'typing-command',
      query: 'reload',
      anchorIndex: 0,
    });
    expect(result.current.commandResults).toHaveLength(1);
    expect(result.current.commandResults[0].id).toBe('/reload');
    expect(result.current.commandResults[0].label).toBe('/reload');
  });

  it('should not identify a command when / is preceded by a non-whitespace character', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('hello/clear', 11);
    });

    expect(result.current.stage).toEqual({ kind: 'idle' });
    expect(result.current.commandResults).toHaveLength(0);
  });

  it('should not identify a command for path slashes', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('src/features/agent', 18);
    });

    expect(result.current.stage).toEqual({ kind: 'idle' });
    expect(result.current.commandResults).toHaveLength(0);
  });

  it('keeps filtered result array references stable across parent re-renders', () => {
    const { result, rerender } = renderHook(
      ({ skills, tools }) => useInputToken(skills, tools),
      {
        initialProps: { skills: mockSkills, tools: mockTools },
      },
    );

    act(() => {
      result.current.onInputChange('@skill:', 7);
    });

    const skillResultsBefore = result.current.skillResults;
    expect(skillResultsBefore).toHaveLength(2);

    // Simulate streaming-driven parent re-render with the same inputs
    rerender({ skills: mockSkills, tools: mockTools });

    expect(result.current.skillResults).toBe(skillResultsBefore);

    act(() => {
      result.current.onInputChange('@tool:', 6);
    });

    const toolResultsBefore = result.current.toolResults;
    expect(toolResultsBefore).toHaveLength(2);

    rerender({ skills: mockSkills, tools: mockTools });

    expect(result.current.toolResults).toBe(toolResultsBefore);

    act(() => {
      result.current.onInputChange('@sk', 3);
    });

    const typeResultsBefore = result.current.typeResults;
    expect(typeResultsBefore.length).toBeGreaterThan(0);

    rerender({ skills: mockSkills, tools: mockTools });

    expect(typeResultsBefore).toBe(result.current.typeResults);
  });

  it('merges plugin commands (bare + namespaced) into command results (#2922)', () => {
    const { result } = renderHook(() =>
      useInputToken([], [], mockPluginCommands),
    );

    act(() => {
      result.current.onInputChange('/hello', 6);
    });

    const ids = result.current.commandResults.map((c) => c.id);
    expect(ids).toContain('/hello');
    expect(ids).toContain('/alpha:hello');
    const bare = result.current.commandResults.find((c) => c.id === '/hello');
    expect(bare?.label).toBe('/hello <name>');
    expect(bare?.description).toBe('say hi');
  });

  it('filters namespaced-only plugin commands while typing the colon (#2922)', () => {
    const { result } = renderHook(() =>
      useInputToken([], [], mockPluginCommands),
    );

    act(() => {
      result.current.onInputChange('/alpha:', 7);
    });

    const ids = result.current.commandResults.map((c) => c.id);
    expect(ids).toEqual(['/alpha:hello']);
    expect(result.current.commandResults[0].description).toBe('say hi');
  });

  it('falls back to a plugin-sourced description when the command has none (#2922)', () => {
    const { result } = renderHook(() =>
      useInputToken([], [], mockPluginCommands),
    );

    act(() => {
      result.current.onInputChange('/beta:deploy', 12);
    });

    const deploy = result.current.commandResults.find(
      (c) => c.id === '/beta:deploy',
    );
    expect(deploy?.description).toBe('Plugin command from beta');
  });
});
