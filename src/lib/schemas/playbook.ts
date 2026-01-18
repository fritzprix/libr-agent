import { z } from 'zod';

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
  requiredData: z.array(z.string()),
  outputVariable: z.string(),
});

/**
 * Schema for playbook workflow structure
 */
export const PlaybookWorkflowSchema = z.object({
  steps: z.array(PlaybookStepSchema),
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

/**
 * Helper function to safely parse playbook workflow JSON
 */
export function parsePlaybookWorkflow(json: string): PlaybookWorkflow {
  const parsed = JSON.parse(json);
  return PlaybookWorkflowSchema.parse(parsed);
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
    const result = PlaybookWorkflowSchema.safeParse(parsed);
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
