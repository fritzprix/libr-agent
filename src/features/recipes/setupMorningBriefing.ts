import { createId } from '@paralleldrive/cuid2';
import { buildServerEntityFromPreset } from '@/features/mcp-servers/utils/preset-utils';
import { humanizeVerificationError } from '@/features/mcp-servers/utils/verification-feedback';
import {
  createAssistant,
  getAssistant,
  updateAssistant,
} from '@/lib/backend/assistants';
import { safeInvoke } from '@/lib/backend/core';
import {
  listMCPServerPresets,
  type MCPServerPreset,
} from '@/lib/backend/mcp-server-config';
import {
  createScheduledTask,
  updateScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { getLogger } from '@/lib/logger';
import type { MCPServerEntity } from '@/models/chat';
import { MORNING_BRIEFING_RECIPE } from './builtin-recipes';

const logger = getLogger('setupMorningBriefing');

export const FALLBACK_PRESETS: Record<string, MCPServerPreset> = {
  hn: {
    name: 'hn',
    category: 'search',
    description:
      'Read Hacker News — top, new, best, Ask HN, Show HN, and job stories.',
    transportType: 'stdio',
    command: 'npx',
    args: ['-y', '@fre4x/hn'],
  },
  'yahoo-finance': {
    name: 'yahoo-finance',
    category: 'data',
    description:
      'Get real-time stock prices, financials, options chains, and insider data. No API key required.',
    transportType: 'stdio',
    command: 'npx',
    args: ['-y', '@fre4x/yahoo-finance'],
  },
};

export type MorningBriefingTranslate = (
  key: string,
  options?: { defaultValue?: string },
) => string;

export interface SetupMorningBriefingDeps {
  saveServer: (server: MCPServerEntity) => Promise<MCPServerEntity>;
  selectedPresets: Record<string, boolean>;
  t: MorningBriefingTranslate;
}

export interface SetupMorningBriefingResult {
  assistantId: string;
  testRunPrompt: string;
}

async function verifyInstalledServers(serverIds: string[]): Promise<void> {
  for (const serverId of serverIds) {
    try {
      await safeInvoke('probe_mcp_server', { serverId });
    } catch (error) {
      const raw = error instanceof Error ? error.message : String(error);
      const humanized = humanizeVerificationError(raw);
      throw new Error(humanized?.summary ?? raw);
    }
  }
}

async function resolveInstalledServerIds(
  selectedPresets: Record<string, boolean>,
  saveServer: SetupMorningBriefingDeps['saveServer'],
): Promise<string[]> {
  let presets: MCPServerPreset[] = [];
  try {
    presets = await listMCPServerPresets();
  } catch (err) {
    logger.warn('Failed to fetch presets via IPC, using fallback', err);
  }

  const existingServers =
    (await safeInvoke<Array<{ id: string; name: string }>>(
      'list_mcp_server_configs',
    )) ?? [];

  const installedServerIds: string[] = [];
  for (const item of MORNING_BRIEFING_RECIPE.requiredMcpPresets) {
    if (!selectedPresets[item.presetName]) continue;

    const existingServer = existingServers.find(
      (server) => server.name === item.presetName,
    );
    if (existingServer?.id) {
      installedServerIds.push(existingServer.id);
      continue;
    }

    const preset =
      presets.find((p) => p.name === item.presetName) ||
      FALLBACK_PRESETS[item.presetName];

    if (preset) {
      const entity = buildServerEntityFromPreset(preset, createId());
      const saved = await saveServer(entity);
      if (saved?.id) {
        installedServerIds.push(saved.id);
      }
    }
  }

  return installedServerIds;
}

async function upsertBriefingAssistant(params: {
  t: MorningBriefingTranslate;
  installedServerIds: string[];
}): Promise<{ id: string }> {
  const { t, installedServerIds } = params;

  const localizedAssistantName = t('recipes.morningBriefing.assistantName', {
    defaultValue: MORNING_BRIEFING_RECIPE.assistantTemplate.name,
  });
  const localizedAssistantDesc = t('recipes.morningBriefing.assistantDesc', {
    defaultValue: MORNING_BRIEFING_RECIPE.assistantTemplate.description,
  });
  const localizedAssistantSystemPrompt = t(
    'recipes.morningBriefing.assistantSystemPrompt',
    {
      defaultValue: MORNING_BRIEFING_RECIPE.assistantTemplate.systemPrompt,
    },
  );

  const existingAssistants =
    (await safeInvoke<Array<{ id: string; name: string }>>(
      'list_assistants',
    )) ?? [];
  const existingAssistant = existingAssistants.find(
    (assistant) =>
      assistant.name === localizedAssistantName ||
      assistant.name === MORNING_BRIEFING_RECIPE.assistantTemplate.name,
  );

  if (existingAssistant) {
    let existingFull = null;
    try {
      existingFull = await getAssistant(existingAssistant.id);
    } catch (e) {
      logger.warn('Failed to fetch existing assistant details', e);
    }
    await updateAssistant({
      id: existingAssistant.id,
      name: localizedAssistantName,
      description: localizedAssistantDesc,
      systemPrompt: localizedAssistantSystemPrompt,
      allowedBuiltInServiceAliases:
        MORNING_BRIEFING_RECIPE.assistantTemplate.allowedBuiltInServiceAliases,
      mcpServerIds: installedServerIds,
      deletionProtected: existingFull?.deletionProtected ?? false,
      avatar: existingFull?.avatar,
      disabledSkills: existingFull?.disabledSkills,
      createdAt: existingFull?.createdAt ?? new Date(),
      updatedAt: new Date(),
    });
    return existingAssistant;
  }

  return createAssistant({
    id: createId(),
    name: localizedAssistantName,
    description: localizedAssistantDesc,
    systemPrompt: localizedAssistantSystemPrompt,
    allowedBuiltInServiceAliases:
      MORNING_BRIEFING_RECIPE.assistantTemplate.allowedBuiltInServiceAliases,
    mcpServerIds: installedServerIds,
    deletionProtected: false,
    createdAt: new Date(),
    updatedAt: new Date(),
  });
}

async function upsertBriefingScheduledTask(params: {
  t: MorningBriefingTranslate;
  assistantId: string;
}): Promise<void> {
  const { t, assistantId } = params;

  const localizedTaskName = t('recipes.morningBriefing.scheduledTaskName', {
    defaultValue: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
  });
  const localizedTaskMessage = t(
    'recipes.morningBriefing.scheduledTaskMessage',
    {
      defaultValue: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.message,
    },
  );

  const existingTasks =
    (await safeInvoke<Array<{ id: string; name: string }>>(
      'list_scheduled_tasks',
    )) ?? [];
  const existingTask = existingTasks.find(
    (task) =>
      task.name === localizedTaskName ||
      task.name === MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
  );

  if (existingTask) {
    await updateScheduledTask(existingTask.id, {
      name: localizedTaskName,
      cronExpression:
        MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.cronExpression,
      executionMode:
        MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.executionMode,
      assistantId,
      message: localizedTaskMessage,
    });
    return;
  }

  await createScheduledTask({
    name: localizedTaskName,
    cronExpression:
      MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.cronExpression,
    executionMode: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.executionMode,
    assistantId,
    message: localizedTaskMessage,
  });
}

/**
 * Installs MCP presets, verifies them, and upserts the morning briefing
 * assistant + scheduled task. UI toasts/navigation stay in the caller.
 */
export async function setupMorningBriefingRecipe(
  deps: SetupMorningBriefingDeps,
): Promise<SetupMorningBriefingResult> {
  const { saveServer, selectedPresets, t } = deps;

  const installedServerIds = await resolveInstalledServerIds(
    selectedPresets,
    saveServer,
  );

  if (installedServerIds.length > 0) {
    await verifyInstalledServers(installedServerIds);
  }

  const targetAssistant = await upsertBriefingAssistant({
    t,
    installedServerIds,
  });

  await upsertBriefingScheduledTask({
    t,
    assistantId: targetAssistant.id,
  });

  const testRunPrompt = t('recipes.morningBriefing.testRunPrompt', {
    defaultValue: MORNING_BRIEFING_RECIPE.testRunPrompt,
  });

  return {
    assistantId: targetAssistant.id,
    testRunPrompt,
  };
}
