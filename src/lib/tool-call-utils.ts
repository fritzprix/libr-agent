import type { Message } from '@/models/chat';
import { isMCPErrorContent } from '@/lib/mcp/protocol/content';
import { z } from 'zod';
import { getLogger } from '@/lib/logger';

const logger = getLogger('tool-call-utils');

/**
 * Schema for validating tool arguments.
 * Ensures parsed JSON is an object (not array, null, or primitive).
 */
const ToolArgumentsSchema = z.record(z.unknown());

/**
 * Checks if a tool result message contains an error.
 * Uses the Message.error property for type-safe error detection.
 */
export function hasToolCallError(toolResult?: Message): boolean {
  // Type-safe error detection using Message.error property
  if (toolResult?.error) return true;

  // Fallback: Check if any content item has isError property
  // This handles cases where backend might preserve MCP result structure in content
  if (toolResult?.content?.some(isMCPErrorContent)) {
    return true;
  }

  return false;
}

/**
 * Checks if a tool result message contains a UI resource.
 * UI resources are content items with type 'resource' and a mimeType.
 */
export function hasUIResource(toolResult?: Message): boolean {
  return (
    toolResult?.content?.some(
      (c) => c.type === 'resource' && c.resource?.mimeType,
    ) || false
  );
}

/**
 * ─── Tool Name Utilities ─────────────────────────────────────────────────────
 *
 * All tool-name formatting/parsing lives here.
 * When the builtin naming convention changes, update ONLY this section.
 *
 * Current convention:  <service>__<tool>
 *   e.g.  planning__addScratchpad
 *         mcp_manager__listServers
 *
 * External MCP tools use the same format:  <server>__<tool>
 *   e.g.  github__search_code
 *
 * Builtin services are identified by their canonical service name (no prefix).
 * This set must mirror BuiltinServiceId::from_alias() in Rust (service_id.rs).
 */

/** Canonical builtin service aliases — must mirror BuiltinServiceId::from_alias() in Rust. */
export const BUILTIN_SERVICE_NAMES = new Set([
  'planning',
  'memory',
  'workspace',
  'knowledge',
  'assistant',
  'skills',
  'playbook',
  'attachments',
  'content_store',
  'contentstore',
  'swarm',
  'session_api',
  'ui',
  'browser',
  'bootstrap',
  'mcp_manager',
]);

/** Returns true if the raw tool name belongs to a builtin service.
 * Requires the `server__tool` delimiter — bare service names (e.g. `'planning'`)
 * are NOT considered builtin tools.
 */
export function isBuiltinTool(name: string): boolean {
  const idx = name.indexOf('__');
  if (idx <= 0 || idx + 2 >= name.length) return false;
  const server = name.slice(0, idx);
  return BUILTIN_SERVICE_NAMES.has(server);
}

/**
 * Parses a builtin tool name into its structural parts.
 * Returns null for external MCP tools or names without a known service prefix.
 *
 * @example
 * parseBuiltinToolName("planning__addScratchpad")
 * // → { serviceId: "planning", toolName: "addScratchpad" }
 */
export function parseBuiltinToolName(
  name: string,
): { serviceId: string; toolName: string } | null {
  const idx = name.indexOf('__');
  if (idx === -1) return null;
  const serviceId = name.slice(0, idx);
  const toolName = name.slice(idx + 2);
  if (!BUILTIN_SERVICE_NAMES.has(serviceId) || !toolName) return null;
  return { serviceId, toolName };
}

/**
 * Returns a human-friendly display name for any tool.
 *
 * Builtin:  "planning__addScratchpad"  → "planning / addScratchpad"
 * External: "github__search_code"      → "search_code"
 */
export function parseToolName(fullToolName: string): string {
  const parsed = parseBuiltinToolName(fullToolName);
  if (parsed) {
    return `${parsed.serviceId} / ${parsed.toolName}`;
  }
  return fullToolName.split('__').pop() || fullToolName;
}

/**
 * Safely parses tool call arguments from JSON string.
 * Returns parsed object or wraps raw string on parse error.
 * Uses Zod schema validation to ensure runtime type safety.
 */
export function parseToolArguments(
  argumentsString: string,
): Record<string, unknown> {
  try {
    const parsed = JSON.parse(argumentsString);

    // Validate parsed value is a record/object
    const validated = ToolArgumentsSchema.safeParse(parsed);

    if (validated.success) {
      return validated.data;
    } else {
      // If parsed value is not an object (e.g., array, string, number, null)
      logger.warn(
        'Tool arguments are not an object, wrapping in value property',
        {
          argumentsString,
          parsed,
          error: validated.error.message,
        },
      );
      return { value: parsed };
    }
  } catch (error) {
    // JSON parsing failed
    logger.debug(
      'Failed to parse tool arguments as JSON, wrapping in raw property',
      {
        argumentsString,
        error: error instanceof Error ? error.message : String(error),
      },
    );
    return { raw: argumentsString };
  }
}

/**
 * Formats execution time in milliseconds to human-readable string.
 * Returns "Xms" for times under 1 second, "X.Xs" otherwise.
 */
export function formatExecutionTime(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * Creates a compact summary string of tool arguments.
 * Example: { path: "./src", recursive: true } -> "path: ./src, recursive: true"
 */
export function formatToolArgumentsSummary(
  args: Record<string, unknown>,
  maxLength: number = 50,
): string {
  if (!args || Object.keys(args).length === 0) return '';

  const summary = Object.entries(args)
    .map(([key, value]) => {
      const valueStr =
        typeof value === 'object' ? JSON.stringify(value) : String(value);
      return `${key}: ${valueStr}`;
    })
    .join(', ');

  if (summary.length <= maxLength) return summary;
  return summary.slice(0, maxLength) + '...';
}
