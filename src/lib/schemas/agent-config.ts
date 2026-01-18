import { z } from 'zod';

/**
 * Schema for agent configuration
 */
export const AgentConfigSchema = z.object({
  name: z.string().optional(),
  systemPrompt: z.string().optional(),
  temperature: z.number().min(0).max(2).optional(),
  maxTokens: z.number().positive().optional(),
  enabledTools: z.array(z.string()).optional(),
  toolConfigs: z.record(z.unknown()).optional(),
  model: z
    .object({
      provider: z.string(),
      modelId: z.string(),
    })
    .optional(),
  advancedSettings: z.record(z.unknown()).optional(),
});

/**
 * Type-safe agent config type derived from schema
 */
export type AgentConfig = z.infer<typeof AgentConfigSchema>;

/**
 * Helper function to safely parse agent config JSON
 */
export function parseAgentConfig(json: string): AgentConfig {
  const parsed = JSON.parse(json);
  return AgentConfigSchema.parse(parsed);
}

/**
 * Safe parse with error handling that returns empty object on failure
 */
export function safeParseAgentConfig(json: string): AgentConfig {
  try {
    const parsed = JSON.parse(json);
    const result = AgentConfigSchema.safeParse(parsed);
    return result.success ? result.data : {};
  } catch {
    return {};
  }
}
