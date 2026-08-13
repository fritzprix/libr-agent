import { z } from 'zod';
import { resolveStructuredToolKey } from './types';

const LatestMessageSchema = z.object({
  role: z.string(),
  summary: z.string(),
  createdAt: z.number().optional(),
});

/**
 * Shared shape for agent__* structured_content payloads.
 * Backend fields are camelCase; optional keys vary by tool / outcome.
 */
export const AgentSessionToolResultSchema = z
  .object({
    toolName: z.string().optional(),
    resourceType: z.string().optional(),
    resourceId: z.string().optional(),
    message: z.string().optional(),
    responseStatus: z.string().optional(),
    sessionId: z.string().optional(),
    /**
     * Opaque DB/storage key for UI navigation. Prefer over `sessionId` when opening
     * a session route — display tokens may differ from storage for legacy ids.
     */
    storageSessionId: z.string().optional(),
    status: z.string().optional(),
    turnCount: z.number().optional(),
    workspacePath: z.string().optional(),
    /** True when caller passed workspaceOverride explicitly. */
    workspaceOverride: z.boolean().optional(),
    assistantName: z.string().optional(),
    assistantId: z.string().optional(),
    /** Mission text from startSession (also may come from tool args). */
    task: z.string().optional(),
    /** Instruction text from messageToSession. */
    instruction: z.string().optional(),
    result: z.string().optional(),
    timeout: z.boolean().optional(),
    timeoutSeconds: z.number().optional(),
    latestMessages: z.array(LatestMessageSchema).optional(),
    messageId: z.string().optional(),
    stopped: z.boolean().optional(),
    deleted: z.boolean().optional(),
    deletedIds: z.array(z.string()).optional(),
    descendantCount: z.number().optional(),
    abnormalTermination: z.boolean().optional(),
    recoverable: z.boolean().optional(),
    hasMoreDetail: z.boolean().optional(),
    orgId: z.string().optional(),
  })
  .passthrough();

export type AgentSessionToolResult = z.infer<
  typeof AgentSessionToolResultSchema
>;

export type AgentSessionCardKind =
  | 'spawned'
  | 'instruction_sent'
  | 'in_progress'
  | 'wait_timeout'
  | 'finished'
  | 'needs_attention'
  | 'stopped'
  | 'deleted';

const AGENT_SESSION_TOOLS = new Set([
  'agent__startSession',
  'agent__messageToSession',
  'agent__checkSession',
  'agent__stopSession',
  'agent__deleteSession',
]);

export function isAgentSessionStructuredTool(toolName: string): boolean {
  return AGENT_SESSION_TOOLS.has(resolveStructuredToolKey(toolName));
}

export function parseAgentSessionToolResult(
  value: unknown,
): AgentSessionToolResult | null {
  const parsed = AgentSessionToolResultSchema.safeParse(value);
  if (!parsed.success) return null;
  const data = parsed.data;
  // Require at least a session reference or delete/stop marker so random objects fail closed.
  if (
    !data.sessionId &&
    !data.resourceId &&
    data.deleted !== true &&
    data.stopped === undefined
  ) {
    return null;
  }
  return data;
}

function statusKey(status: string | undefined): string {
  return (status ?? '').toLowerCase();
}

function isAttentionStatus(status: string | undefined): boolean {
  return ['paused', 'error', 'failed', 'terminated'].includes(
    statusKey(status),
  );
}

function isFinishedStatus(status: string | undefined): boolean {
  return statusKey(status) === 'idle';
}

/**
 * Map tool name + structured payload to a human card kind.
 * Wait/poll are agent parameters — kinds describe outcomes, not user actions.
 *
 * Harvest/attention require real session lifecycle status (`idle` / `paused` /
 * `error` / …). Do not treat `responseStatus === 'success'` alone as finished —
 * stop/delete/spawn envelopes can also carry that string.
 */
export function classifyAgentSessionCard(
  toolName: string,
  data: AgentSessionToolResult,
): AgentSessionCardKind | null {
  const key = resolveStructuredToolKey(toolName);
  if (!AGENT_SESSION_TOOLS.has(key)) return null;

  if (key === 'agent__deleteSession') {
    // Require explicit deleted marker — do not infer from responseStatus alone.
    return data.deleted === true ? 'deleted' : null;
  }

  if (key === 'agent__stopSession') {
    // Successful stop sets status=terminated; only treat explicit error as failure.
    if (data.responseStatus === 'error') {
      return 'needs_attention';
    }
    return 'stopped';
  }

  if (data.timeout === true || data.responseStatus === 'timeout') {
    return 'wait_timeout';
  }

  // Settled child outcomes — driven by session status, not responseStatus alone.
  // (startSession/messageToSession wait=* reuse checkSession-shaped payloads.)
  if (isAttentionStatus(data.status)) {
    return 'needs_attention';
  }
  if (isFinishedStatus(data.status)) {
    return 'finished';
  }

  if (key === 'agent__startSession') {
    return 'spawned';
  }
  if (key === 'agent__messageToSession') {
    return 'instruction_sent';
  }
  if (key === 'agent__checkSession') {
    return 'in_progress';
  }

  return null;
}

export function resolveAgentSessionId(
  data: AgentSessionToolResult,
): string | null {
  // Prefer storage key for Open Session navigation; fall back to display/resource ids.
  const id =
    data.storageSessionId?.trim() ||
    data.sessionId?.trim() ||
    data.resourceId?.trim();
  return id && id.length > 0 ? id : null;
}

export function readAgentToolArgString(
  toolArgs: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = toolArgs?.[key];
  return typeof value === 'string' && value.trim().length > 0
    ? value.trim()
    : undefined;
}
