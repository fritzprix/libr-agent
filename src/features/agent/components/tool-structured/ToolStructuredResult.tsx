import React from 'react';
import { FileWriteActions } from './FileWriteActions';
import { StrReplaceDiffView } from './StrReplaceDiffView';
import { TerminalOutputBlock } from './TerminalOutputBlock';
import { AgentSessionToolCard } from './AgentSessionToolCard';
import {
  classifyAgentSessionCard,
  isAgentSessionStructuredTool,
  parseAgentSessionToolResult,
} from './agent-types';
import {
  parseRunShellResult,
  parseStrReplaceResult,
  parseWriteFileResult,
  resolveStructuredToolKey,
} from './types';

export interface ToolStructuredResultProps {
  toolName: string;
  data: unknown;
  /** Optional tool-call arguments (e.g. task / message for agent cards). */
  toolArgs?: Record<string, unknown>;
}

/**
 * Dispatches MCP structured_content to a typed tool result view.
 * Returns null when the tool is unsupported or the payload fails validation.
 *
 * Extending for a new tool:
 * 1. Add a Zod schema + parse* helper in `./types` (or domain module)
 * 2. Add a `case` here that renders the view
 * 3. Register the same key in {@link canRenderStructuredToolResult}
 * Visibility override (always-visible compact UI) follows automatically via
 * {@link resolveToolResultUiOverride}.
 */
export const ToolStructuredResult: React.FC<ToolStructuredResultProps> = ({
  toolName,
  data,
  toolArgs,
}) => {
  const key = resolveStructuredToolKey(toolName);

  switch (key) {
    case 'workspace__writeFile': {
      const parsed = parseWriteFileResult(data);
      return parsed ? <FileWriteActions data={parsed} /> : null;
    }
    case 'workspace__strReplace': {
      const parsed = parseStrReplaceResult(data);
      return parsed ? <StrReplaceDiffView data={parsed} /> : null;
    }
    case 'workspace__runShell': {
      const parsed = parseRunShellResult(data);
      return parsed ? <TerminalOutputBlock data={parsed} /> : null;
    }
    case 'agent__startSession':
    case 'agent__messageToSession':
    case 'agent__checkSession':
    case 'agent__stopSession':
    case 'agent__deleteSession': {
      const parsed = parseAgentSessionToolResult(data);
      if (!parsed || !classifyAgentSessionCard(toolName, parsed)) {
        return null;
      }
      return (
        <AgentSessionToolCard
          toolName={toolName}
          data={parsed}
          toolArgs={toolArgs}
        />
      );
    }
    default:
      return null;
  }
};

/**
 * Returns true when `data` can be rendered by {@link ToolStructuredResult}
 * for the given tool name. Used to suppress duplicate markdown summaries.
 */
export function canRenderStructuredToolResult(
  toolName: string,
  data: unknown,
): boolean {
  const key = resolveStructuredToolKey(toolName);
  switch (key) {
    case 'workspace__writeFile':
      return parseWriteFileResult(data) !== null;
    case 'workspace__strReplace':
      return parseStrReplaceResult(data) !== null;
    case 'workspace__runShell':
      return parseRunShellResult(data) !== null;
    default: {
      if (!isAgentSessionStructuredTool(toolName)) return false;
      const parsed = parseAgentSessionToolResult(data);
      return (
        parsed !== null && classifyAgentSessionCard(toolName, parsed) !== null
      );
    }
  }
}
