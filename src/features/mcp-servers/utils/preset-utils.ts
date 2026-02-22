import { MCPServerEntity } from '@/models/chat';
import { type MCPServerPreset } from '@/lib/backend/mcp-server-config';

export function sanitizePresetEnv(
  env: MCPServerPreset['env'],
): Record<string, string> {
  if (!env) {
    return {};
  }

  return Object.entries(env).reduce<Record<string, string>>(
    (accumulator, [key, value]) => {
      if (typeof value === 'string') {
        accumulator[key] = value;
      }
      return accumulator;
    },
    {},
  );
}

export function buildPresetMetadata(
  preset: MCPServerPreset,
): MCPServerEntity['metadata'] {
  return {
    description: preset.description,
    variableDefinitions: preset.variableDefinitions,
  };
}
