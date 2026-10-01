import { createId } from '@paralleldrive/cuid2';
import {
  listMCPServerPresets,
  type MCPServerPreset,
} from '@/lib/backend/mcp-server-config';
import type { MCPServerEntity } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import {
  buildServerEntityFromPreset,
  presetNeedsUserConfig,
} from '@/features/mcp-servers/utils/preset-utils';

const logger = getLogger('app-control');

function normalizeName(value: string): string {
  // Backend sanitize_mcp_server_name maps '-' → '_'; match that for already-installed checks.
  return value.trim().toLowerCase().replace(/-/g, '_');
}

function findPreset(
  presets: MCPServerPreset[],
  name: string,
): MCPServerPreset | undefined {
  const needle = normalizeName(name);
  return presets.find((preset) => normalizeName(preset.name) === needle);
}

/**
 * One-click install a zero-config registry preset by name (case-insensitive).
 * Throws if the preset is missing or requires user configuration.
 */
export async function installPresetByName(
  name: string,
  saveServer: (server: MCPServerEntity) => Promise<MCPServerEntity>,
  installedServers: readonly MCPServerEntity[],
): Promise<MCPServerEntity> {
  const presets = await listMCPServerPresets();
  const preset = findPreset(presets, name);
  if (!preset) {
    const available = presets.map((p) => p.name).join(', ');
    throw new Error(
      `Preset not found: "${name}". Available: ${available || '(none)'}`,
    );
  }

  if (presetNeedsUserConfig(preset)) {
    throw new Error(
      `Preset "${preset.name}" requires user configuration (API keys/OAuth) and cannot be one-click installed via app control.`,
    );
  }

  const already = installedServers.find(
    (server) => normalizeName(server.name) === normalizeName(preset.name),
  );
  if (already) {
    logger.info(`Preset "${preset.name}" already installed (${already.id})`);
    return already;
  }

  const entity = buildServerEntityFromPreset(preset, createId());
  const saved = await saveServer(entity);
  logger.info(`Installed preset "${preset.name}" as ${saved.id}`);
  return saved;
}
