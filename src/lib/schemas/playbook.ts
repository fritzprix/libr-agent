import { z } from 'zod';
import { getLogger } from '@/lib/logger';

const logger = getLogger('PlaybookSchema');

/**
 * Schema for playbook workflow steps.
 * Aligns with Rust `PlaybookStep`: `stepId` / `requiredData` are optional.
 */
export const PlaybookStepSchema = z.object({
  stepId: z.string().optional(),
  description: z.string(),
  action: z.object({
    toolName: z.string(),
    purpose: z.string(),
  }),
  requiredData: z.array(z.string()).optional().default([]),
  outputVariable: z.string(),
});

/**
 * Minimal Start launch target (pin sessionId for Card Start).
 * Discriminated union matches TS `PlaybookDefaultTargetSession` and Rust pin rules.
 */
export const PlaybookDefaultTargetSessionSchema = z.discriminatedUnion('mode', [
  z.object({
    mode: z.literal('self'),
    sessionId: z.string().trim().min(1).optional(),
  }),
  z.object({
    mode: z.literal('pin'),
    sessionId: z.string().trim().min(1),
  }),
]);

/**
 * Schema for playbook workflow structure
 */
export const PlaybookWorkflowSchema = z.object({
  steps: z.array(PlaybookStepSchema),
  defaultTargetSession: PlaybookDefaultTargetSessionSchema.optional(),
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
 * Type-safe playbook workflow type derived from schema
 */
export type PlaybookWorkflow = z.infer<typeof PlaybookWorkflowSchema>;

/**
 * Type-safe success criteria type derived from schema
 */
export type SuccessCriteria = z.infer<typeof SuccessCriteriaSchema>;

function parseSteps(
  stepsValue: unknown,
  context: string,
): z.infer<typeof PlaybookStepSchema>[] | undefined {
  const stepsResult = z.array(PlaybookStepSchema).safeParse(stepsValue);
  if (stepsResult.success) {
    return stepsResult.data;
  }
  logger.warn('Failed to parse playbook workflow steps', {
    context,
    issues: stepsResult.error.issues,
  });
  return undefined;
}

function parseDefaultTargetSession(
  value: unknown,
): PlaybookWorkflow['defaultTargetSession'] {
  if (value == null) {
    return undefined;
  }
  const result = PlaybookDefaultTargetSessionSchema.safeParse(value);
  if (result.success) {
    return result.data;
  }
  logger.warn('Ignoring invalid defaultTargetSession', {
    issues: result.error.issues,
  });
  return undefined;
}

/**
 * Helper function to safely parse playbook workflow JSON
 */
export function parsePlaybookWorkflow(json: string): PlaybookWorkflow {
  const parsed = JSON.parse(json);
  const normalized = normalizePlaybookWorkflow(parsed);
  if (!normalized) {
    // Fall back to strict schema parse for actionable Zod errors
    return PlaybookWorkflowSchema.parse(
      Array.isArray(parsed) ? { steps: parsed } : parsed,
    );
  }
  return normalized;
}

/**
 * Helper function to safely parse success criteria JSON
 */
export function parseSuccessCriteria(json: string): SuccessCriteria {
  const parsed = JSON.parse(json);
  return SuccessCriteriaSchema.parse(parsed);
}

/**
 * Normalize legacy raw-array workflow JSON or envelope `{ steps, defaultTargetSession }`.
 * Invalid pin targets are dropped (with warn) without discarding valid steps.
 */
export function normalizePlaybookWorkflow(
  parsed: unknown,
): PlaybookWorkflow | undefined {
  if (Array.isArray(parsed)) {
    const steps = parseSteps(parsed, 'legacy-array');
    return steps ? { steps } : undefined;
  }

  if (!parsed || typeof parsed !== 'object') {
    return undefined;
  }

  const record = parsed as Record<string, unknown>;
  const steps = parseSteps(record.steps, 'envelope');
  if (!steps) {
    return undefined;
  }

  const workflow: PlaybookWorkflow = { steps };

  if ('defaultTargetSession' in record) {
    workflow.defaultTargetSession = parseDefaultTargetSession(
      record.defaultTargetSession,
    );
  }
  if (typeof record.version === 'string') {
    workflow.version = record.version;
  }
  if (
    record.metadata &&
    typeof record.metadata === 'object' &&
    !Array.isArray(record.metadata)
  ) {
    workflow.metadata = record.metadata as Record<string, unknown>;
  }

  return workflow;
}

/**
 * Safe parse with error handling that returns undefined on failure
 */
export function safeParsePlaybookWorkflow(
  json: string,
): PlaybookWorkflow | undefined {
  try {
    const parsed: unknown = JSON.parse(json);
    return normalizePlaybookWorkflow(parsed);
  } catch (error) {
    logger.warn('Failed to parse playbook workflow JSON', { error });
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
