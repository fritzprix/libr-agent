import type { Message } from '@/models/chat';
import { getToolStructuredContent, hasUIResource } from '@/lib/tool-call-utils';
import { canRenderStructuredToolResult } from './ToolStructuredResult';
import { isCheckSessionWaitTool } from './types';

export type ToolResultDetailMode = 'simple' | 'developer';

/**
 * Presentation override for tool results that should bypass compact collapse.
 * New structured UI tools automatically opt in via {@link canRenderStructuredToolResult}.
 */
export type ToolResultUiOverride = {
  /** Bypass compact collapse; details stay visible */
  alwaysVisible: true;
  /** Hide the parameters section (typical for simple-mode inline views) */
  hideParameters: boolean;
};

/**
 * Resolves whether a tool result should override the default collapsed compact UI.
 *
 * Order:
 * 1. In-flight checkSession(wait=true) — waiting chrome + Stop
 * 2. MCP UI resources (e.g. circuit-break widgets)
 * 3. Structured content with a registered renderer
 * Otherwise returns null → keep default collapse behavior.
 */
export function resolveToolResultUiOverride(
  toolName: string,
  toolResult: Message | undefined,
  mode: ToolResultDetailMode,
  parsedArgs?: Record<string, unknown>,
): ToolResultUiOverride | null {
  const hideParameters = mode === 'simple';

  if (
    !toolResult &&
    parsedArgs &&
    isCheckSessionWaitTool(toolName, parsedArgs)
  ) {
    return { alwaysVisible: true, hideParameters };
  }

  if (!toolResult) return null;

  if (hasUIResource(toolResult)) {
    return { alwaysVisible: true, hideParameters };
  }

  const structured = getToolStructuredContent(toolResult);
  if (
    structured !== undefined &&
    canRenderStructuredToolResult(toolName, structured)
  ) {
    // mode only controls hideParameters — never skip alwaysVisible for simple.
    return { alwaysVisible: true, hideParameters };
  }

  return null;
}
