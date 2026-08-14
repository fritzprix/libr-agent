import { useState, useCallback, useRef, useMemo } from 'react';
import type { SkillMetadata } from '@/types/skills';
import type { MCPTool } from '@/lib/mcp';
import type { Playbook } from '@/types/playbook';

/** Stage of the input token UX state machine. */
type TokenStage =
  | { kind: 'idle' }
  | { kind: 'typing-type'; query: string; anchorIndex: number } // `@query` before `:`
  | {
      kind: 'typing-arg';
      typeName: string;
      query: string;
      anchorIndex: number;
    } // `@type:query`
  | { kind: 'typing-command'; query: string; anchorIndex: number }; // `/query`

/** A reference type shown in the first-level dropdown (e.g. "skill:"). */
export interface TokenType {
  name: string; // e.g. "skill"
  label: string; // e.g. "skill:"
  description: string;
}

export const BUILTIN_TOKEN_TYPES: TokenType[] = [
  {
    name: 'skill',
    label: 'skill:',
    description: 'Inject skill documentation into context',
  },
  {
    name: 'file',
    label: 'file:',
    description: 'Reference a workspace file without inlining its full content',
  },
  {
    name: 'tool',
    label: 'tool:',
    description: 'Add soft attention hint for a specific tool',
  },
  {
    name: 'playbook',
    label: 'playbook:',
    description: 'Execute a saved playbook workflow',
  },
];

/** Regex: matches `@word` with no `:` yet (typing type name). */
const TYPE_RE = /@([\w]*)$/;
/** Regex: matches `@word:rest` (typing arg for a type). */
const ARG_RE = /@([\w]+):([\S]*)$/;
/** Regex: matches `/command` with potential spaces. */
const COMMAND_RE = /(?<!\S)\/([\w\s-]*)$/;

export interface UseInputTokenResult {
  /** Current UX stage. */
  stage: TokenStage;
  /** Type options to show when stage is `typing-type`. */
  typeResults: TokenType[];
  /** Skill candidates to show when stage is `typing-arg` with type=skill. */
  skillResults: SkillMetadata[];
  /** Tool candidates to show when stage is `typing-arg` with type=tool. */
  toolResults: MCPTool[];
  /** File path candidates to show when stage is `typing-arg` with type=file. */
  fileResults: string[];
  /** Playbook candidates to show when stage is `typing-arg` with type=playbook. */
  playbookResults: (Playbook & {
    id: string;
    createdAt: Date;
    updatedAt: Date;
  })[];
  /** Command candidates to show when stage is `typing-command`. */
  commandResults: { id: string; label: string; description: string }[];
  /** Call on every textarea value+cursor change. */
  onInputChange: (value: string, cursorPos: number) => void;
  /** Select a token type — inserts `@type:` and switches to arg stage. */
  onTypeSelect: (
    typeName: string,
    currentValue: string,
    cursorPos: number,
  ) => string;
  /** Select an arg — inserts `@type:name` into text. */
  onArgSelect: (arg: string, currentValue: string, cursorPos: number) => string;
  /** Dismiss dropdown without selection. */
  onDismiss: () => void;
}

export function useInputToken(
  skills: SkillMetadata[],
  tools: MCPTool[] = [],
): UseInputTokenResult {
  const [stage, setStage] = useState<TokenStage>({ kind: 'idle' });
  // Keep a ref for non-reactive use in callbacks
  const stageRef = useRef<TokenStage>({ kind: 'idle' });

  const setState = useCallback((next: TokenStage) => {
    stageRef.current = next;
    setStage(next);
  }, []);

  const onInputChange = useCallback(
    (value: string, cursorPos: number) => {
      const textBeforeCursor = value.slice(0, cursorPos);

      // Handle slash commands first
      const commandMatch = COMMAND_RE.exec(textBeforeCursor);
      if (commandMatch) {
        setState({
          kind: 'typing-command',
          query: commandMatch[1],
          anchorIndex: commandMatch.index,
        });
        return;
      }

      const argMatch = ARG_RE.exec(textBeforeCursor);
      if (argMatch) {
        setState({
          kind: 'typing-arg',
          typeName: argMatch[1],
          query: argMatch[2],
          anchorIndex: argMatch.index,
        });
        return;
      }

      const typeMatch = TYPE_RE.exec(textBeforeCursor);
      if (typeMatch) {
        setState({
          kind: 'typing-type',
          query: typeMatch[1],
          anchorIndex: typeMatch.index,
        });
        return;
      }

      setState({ kind: 'idle' });
    },
    [setState],
  );

  /** Insert `@type:` into the textarea, replacing the partial `@query`. */
  const onTypeSelect = useCallback(
    (typeName: string, currentValue: string, cursorPos: number): string => {
      const s = stageRef.current;
      if (s.kind !== 'typing-type') return currentValue;
      const before = currentValue.slice(0, s.anchorIndex);
      const after = currentValue.slice(cursorPos);
      const insertion = `@${typeName}:`;
      const newValue = before + insertion + after;
      setState({
        kind: 'typing-arg',
        typeName,
        query: '',
        anchorIndex: s.anchorIndex,
      });
      return newValue;
    },
    [setState],
  );

  /** Insert `@type:arg` (replacing the partial token) into the textarea. */
  const onArgSelect = useCallback(
    (arg: string, currentValue: string, cursorPos: number): string => {
      const s = stageRef.current;
      if (s.kind === 'typing-command') {
        const before = currentValue.slice(0, s.anchorIndex);
        const after = currentValue.slice(cursorPos);
        const insertion = `${arg} `;
        setState({ kind: 'idle' });
        return before + insertion + after;
      }
      if (s.kind !== 'typing-arg') return currentValue;
      const before = currentValue.slice(0, s.anchorIndex);
      const after = currentValue.slice(cursorPos);
      const insertion = `@${s.typeName}:${arg} `;
      setState({ kind: 'idle' });
      return before + insertion + after;
    },
    [setState],
  );

  const onDismiss = useCallback(() => {
    setState({ kind: 'idle' });
  }, [setState]);

  // Memoize filtered arrays so reference identity stays stable across parent
  // re-renders (e.g. streaming). InputTokenDropdown passes these to
  // useListNavigation as resetDependencies; a new array each render would
  // reset the keyboard selection to the first item.
  const typeResults = useMemo(() => {
    if (stage.kind !== 'typing-type') return [];
    return BUILTIN_TOKEN_TYPES.filter((t) =>
      t.name.startsWith(stage.query.toLowerCase()),
    );
  }, [stage]);

  const skillResults = useMemo(() => {
    if (stage.kind !== 'typing-arg' || stage.typeName !== 'skill') return [];
    return skills.filter((s) =>
      s.name.toLowerCase().includes(stage.query.toLowerCase()),
    );
  }, [skills, stage]);

  const toolResults = useMemo(() => {
    if (stage.kind !== 'typing-arg' || stage.typeName !== 'tool') return [];
    return tools
      .filter((t) => t.name.toLowerCase().includes(stage.query.toLowerCase()))
      .slice(0, 10);
  }, [tools, stage]);

  // Filter file results (pre-filtered by useWorkspaceFiles; just pass through as empty — handled by consumer)
  const fileResults: string[] = [];

  // Playbook results are pre-filtered by usePlaybookSearch; pass through as empty — handled by consumer
  const playbookResults: (Playbook & {
    id: string;
    createdAt: Date;
    updatedAt: Date;
  })[] = [];

  // Filter slash command results
  const commandResults = useMemo(() => {
    const commands = [
      {
        id: '/clear',
        label: '/clear',
        description:
          'Reset session (clear messages cache and database history)',
      },
      {
        id: '/permission yolo',
        label: '/permission yolo',
        description: 'Execute tools automatically without requiring approval',
      },
      {
        id: '/permission unsafe',
        label: '/permission unsafe',
        description: 'Bypass standard approval and policy verification',
      },
      {
        id: '/permission normal',
        label: '/permission normal',
        description: 'Require standard approval for tool executions',
      },
    ];
    if (stage.kind === 'typing-command') {
      return commands.filter((c) =>
        c.label.toLowerCase().includes(stage.query.toLowerCase()),
      );
    }
    return [];
  }, [stage]);

  return {
    stage,
    typeResults,
    skillResults,
    toolResults,
    fileResults,
    playbookResults,
    commandResults,
    onInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
  };
}
