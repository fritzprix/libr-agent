import { MCPServerEntity } from '@/models/chat';
import { type MCPServerPreset } from '@/lib/backend/mcp-server-config';
import type { MCPServerMetadata } from '../hooks/useMCPServerForm';

export type VariableDefinitionTarget =
  | 'env'
  | 'header'
  | 'bearer-token'
  | 'url-param';

export interface VariableDefinitionLike {
  required?: boolean;
  target?: VariableDefinitionTarget;
}

/**
 * Empty strings and YOUR_* style placeholders are not real defaults — the user
 * must still provide a value on the main install surface.
 */
export function isMeaningfulPrefill(value: string | undefined | null): boolean {
  if (typeof value !== 'string') {
    return false;
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  if (/^YOUR_[A-Z0-9_]+$/i.test(trimmed)) {
    return false;
  }
  if (/^Bearer\s+YOUR_[A-Z0-9_]+$/i.test(trimmed)) {
    return false;
  }
  return true;
}

function readPresetEnvMap(
  env: MCPServerPreset['env'] | undefined,
): Record<string, string> {
  return sanitizePresetEnv(env);
}

/**
 * Resolve the raw template value for a variableDefinition from a registry preset.
 */
export function resolvePresetVariableRawValue(
  preset: MCPServerPreset,
  key: string,
  def: VariableDefinitionLike,
): string {
  const env = readPresetEnvMap(preset.env);
  const target = def.target ?? 'env';

  if (target === 'bearer-token') {
    const auth = env.Authorization ?? env.authorization ?? '';
    if (auth.toLowerCase().startsWith('bearer ')) {
      return auth.slice(7).trim();
    }
    return env[key] ?? '';
  }

  if (target === 'header') {
    return env[key] ?? '';
  }

  if (target === 'url-param') {
    if (!preset.url) {
      return '';
    }
    try {
      return new URL(preset.url).searchParams.get(key) ?? '';
    } catch {
      return '';
    }
  }

  return env[key] ?? '';
}

/**
 * Snapshot meaningful defaults from mcp-server.json so UI visibility stays
 * data-driven after install (does not follow live typed values).
 */
export function collectMeaningfulVariableDefaults(
  preset: MCPServerPreset,
): Record<string, string> {
  const definitions = preset.variableDefinitions;
  if (!definitions) {
    return {};
  }

  const defaults: Record<string, string> = {};
  for (const [key, def] of Object.entries(definitions)) {
    const raw = resolvePresetVariableRawValue(preset, key, def);
    if (isMeaningfulPrefill(raw)) {
      defaults[key] = raw;
    }
  }
  return defaults;
}

/**
 * True when this variable should live under Advanced Settings.
 * Prefill map wins; legacy installs without the map fall back to inferring from
 * the entity template values (still not per-MCP branching).
 */
export function isPrefilledVariableDefinition(
  key: string,
  def: VariableDefinitionLike,
  metadata: MCPServerMetadata | undefined,
  fallbackServer?: MCPServerEntity,
): boolean {
  const defaults = metadata?.variableDefaults;
  if (defaults) {
    return Object.prototype.hasOwnProperty.call(defaults, key);
  }

  if (!fallbackServer) {
    return false;
  }

  return isMeaningfulPrefill(
    resolveEntityVariableRawValue(fallbackServer, key, def),
  );
}

export function resolveEntityVariableRawValue(
  server: MCPServerEntity,
  key: string,
  def: VariableDefinitionLike,
): string {
  const target = def.target ?? 'env';
  const transport = server.transport;

  if ((transport.type as string) === 'http' || transport.type === 'http-sse') {
    const headers =
      'headers' in transport && transport.headers ? transport.headers : {};

    if (target === 'bearer-token') {
      const auth = headers.Authorization ?? '';
      if (auth.toLowerCase().startsWith('bearer ')) {
        return auth.slice(7).trim();
      }
      return '';
    }

    if (target === 'header') {
      return headers[key] ?? '';
    }

    if (target === 'url-param' && 'url' in transport && transport.url) {
      try {
        return new URL(transport.url).searchParams.get(key) ?? '';
      } catch {
        return '';
      }
    }
  }

  if (transport.type === 'stdio' && transport.env) {
    const value = transport.env[key];
    return typeof value === 'string' ? value : '';
  }

  return '';
}

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
    source: 'registry',
    category: preset.category,
    description: preset.description,
    logo: preset.logo,
    variableDefinitions: preset.variableDefinitions,
    variableDefaults: collectMeaningfulVariableDefaults(preset),
  };
}

/**
 * True when the server came from mcp-server.json (or still matches a registry preset name).
 * Legacy installs without `metadata.source` are detected via the preset name set.
 */
export function isRegistrySourcedServer(
  server: MCPServerEntity,
  registryPresetNames?: ReadonlySet<string>,
): boolean {
  if (server.metadata?.source === 'registry') {
    return true;
  }
  if (registryPresetNames?.has(server.name)) {
    return true;
  }
  return false;
}
