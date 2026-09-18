import { z } from 'zod';

/**
 * Schema for session targeting mode
 */
export const TargetSessionModeSchema = z.enum(['self', 'pin', 'spawn']);
export type TargetSessionMode = z.infer<typeof TargetSessionModeSchema>;

/**
 * Schema for target session configuration
 */
export const TargetSessionConfigSchema = z.object({
  /** Target execution mode: 'self' (current), 'pin' (existing session), 'spawn' (new session) */
  mode: TargetSessionModeSchema,
  /** Session ID when mode is 'pin' (can also be provided at runtime) */
  sessionId: z.string().optional(),
  /** Assistant configuration ID when mode is 'spawn' */
  configId: z.string().optional(),
  /** Logical session slot name for cross-step session reuse */
  sessionSlot: z.string().optional(),
});

/**
 * Schema for session slot definition
 */
export const SessionSlotConfigSchema = z.object({
  /** Session creation or attachment mode */
  mode: TargetSessionModeSchema,
  /** Assistant configuration template ID to spawn for this slot */
  configId: z.string().optional(),
  /** Pre-pinned session ID for this slot */
  sessionId: z.string().optional(),
  /** Whether a spawned session for this slot should be reused across subsequent steps */
  reuseAcrossSteps: z.boolean().optional(),
});

/**
 * Schema for playbook workflow steps
 */
export const PlaybookStepSchema = z.object({
  stepId: z.string(),
  description: z.string(),
  action: z.object({
    toolName: z.string(),
    purpose: z.string(),
  }),
  targetSession: TargetSessionConfigSchema.optional(),
  sessionSlot: z.string().optional(),
  promptTemplate: z.string().optional(),
  requiredData: z.array(z.string()),
  outputVariable: z.string(),
});

/**
 * Schema for playbook workflow structure
 */
export const PlaybookWorkflowSchema = z.object({
  steps: z.array(PlaybookStepSchema),
  defaultTargetSession: TargetSessionConfigSchema.optional(),
  sessionSlots: z.record(SessionSlotConfigSchema).optional(),
  metadata: z.record(z.unknown()).optional(),
  version: z.string().optional(),
});

/**
 * Schema for playbook success criteria
 */
export const SuccessCriteriaSchema = z.object({
  description: z.string(),
  requiredArtifacts: z.array(z.string()).optional(),
});

/**
 * Type-safe target session config type
 */
export type TargetSessionConfig = z.infer<typeof TargetSessionConfigSchema>;

/**
 * Type-safe session slot config type
 */
export type SessionSlotConfig = z.infer<typeof SessionSlotConfigSchema>;

/**
 * Type-safe playbook step type derived from schema
 */
export type PlaybookStep = z.infer<typeof PlaybookStepSchema>;

/**
 * Type-safe playbook workflow type derived from schema
 */
export type PlaybookWorkflow = z.infer<typeof PlaybookWorkflowSchema>;

/**
 * Type-safe success criteria type derived from schema
 */
export type SuccessCriteria = z.infer<typeof SuccessCriteriaSchema>;

/**
 * Safely normalizes raw JSON data into PlaybookWorkflow object format.
 * If raw data is an array of steps, it wraps it into { steps: rawData }.
 */
function normalizeWorkflowPayload(raw: unknown): unknown {
  if (Array.isArray(raw)) {
    return { steps: raw };
  }
  return raw;
}

/**
 * Helper function to safely parse playbook workflow JSON
 */
export function parsePlaybookWorkflow(json: string): PlaybookWorkflow {
  const parsed = JSON.parse(json);
  return PlaybookWorkflowSchema.parse(normalizeWorkflowPayload(parsed));
}

/**
 * Helper function to safely parse success criteria JSON
 */
export function parseSuccessCriteria(json: string): SuccessCriteria {
  const parsed = JSON.parse(json);
  return SuccessCriteriaSchema.parse(parsed);
}

/**
 * Safe parse with error handling that returns undefined on failure
 */
export function safeParsePlaybookWorkflow(
  json: string,
): PlaybookWorkflow | undefined {
  try {
    const parsed = JSON.parse(json);
    const normalized = normalizeWorkflowPayload(parsed);
    const result = PlaybookWorkflowSchema.safeParse(normalized);
    return result.success ? result.data : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Safe parse with error handling that returns undefined on failure
 */
export function safeParseSuccessCriteria(
  json: string,
): SuccessCriteria | undefined {
  try {
    const parsed = JSON.parse(json);
    const result = SuccessCriteriaSchema.safeParse(parsed);
    return result.success ? result.data : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Interpolate placeholders formatted as {variable_name} in a template string
 * with values from the given variable map.
 */
export function interpolateVariables(
  template: string,
  variables: Record<string, string>,
): string {
  return template.replace(/\{([a-zA-Z0-9_-]+)\}/g, (match, key) => {
    return key in variables ? variables[key] : match;
  });
}
