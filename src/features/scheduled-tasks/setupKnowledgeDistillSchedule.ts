import { listAssistants } from '@/lib/backend/assistants';
import {
  createScheduledTask,
  listScheduledTasks,
  updateScheduledTask,
  type ScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { getLogger } from '@/lib/logger';
import type { Assistant } from '@/models/chat';
import { STARTER_TASK_TEMPLATES } from './starter-templates';

const logger = getLogger('setupKnowledgeDistillSchedule');

const TEMPLATE_ID = 'knowledge-distill';

export type KnowledgeDistillTranslate = (
  key: string,
  options?: { defaultValue?: string },
) => string;

export interface SetupKnowledgeDistillScheduleResult {
  task: ScheduledTask;
  /** true when a new task was created; false when an existing one was updated */
  created: boolean;
  assistantId: string;
}

function getTemplate() {
  const template = STARTER_TASK_TEMPLATES.find((t) => t.id === TEMPLATE_ID);
  if (!template) {
    throw new Error('knowledge-distill starter template is missing');
  }
  return template;
}

function pickAssistant(
  assistants: Assistant[],
  preferredName: string,
): Assistant | undefined {
  if (assistants.length === 0) {
    return undefined;
  }

  const preferred = assistants.find(
    (assistant) => assistant.name === preferredName,
  );
  if (preferred) {
    return preferred;
  }

  const withKnowledgeAndHistory = assistants.find((assistant) => {
    const aliases = assistant.allowedBuiltInServiceAliases ?? [];
    return aliases.includes('knowledge') && aliases.includes('history');
  });
  return withKnowledgeAndHistory ?? assistants[0];
}

/**
 * Upserts the durable knowledge-distillation scheduled task from the starter
 * template. Callers own toasts and navigation.
 */
export async function setupKnowledgeDistillSchedule(deps: {
  t: KnowledgeDistillTranslate;
}): Promise<SetupKnowledgeDistillScheduleResult> {
  const { t } = deps;
  const template = getTemplate();

  const name = t(template.nameKey ?? template.titleKey, {
    defaultValue: template.defaultName || template.name,
  });
  const message = t(template.messageKey ?? '', {
    defaultValue: template.defaultMessage || template.message,
  });

  const assistants = await listAssistants();
  const assistant = pickAssistant(assistants, template.preferredAssistantName);
  if (!assistant?.id) {
    throw new Error(
      t('scheduledTasks.setupKnowledgeDistill.noAssistant', {
        defaultValue:
          'Create an assistant first, then try again.',
      }),
    );
  }

  const existingTasks = await listScheduledTasks();
  const existingTask = existingTasks.find(
    (task) => task.name === name || task.name === template.name,
  );

  if (existingTask) {
    logger.info('Updating existing knowledge distill scheduled task', {
      id: existingTask.id,
    });
    const task = await updateScheduledTask(existingTask.id, {
      name,
      cronExpression: template.cronExpression,
      scheduleTimezone: 'local',
      executionMode: template.executionMode,
      assistantId: assistant.id,
      message,
      resetPlanningState: template.resetPlanningState,
      enabled: true,
    });
    return { task, created: false, assistantId: assistant.id };
  }

  logger.info('Creating knowledge distill scheduled task');
  const task = await createScheduledTask({
    name,
    cronExpression: template.cronExpression,
    scheduleTimezone: 'local',
    executionMode: template.executionMode,
    assistantId: assistant.id,
    message,
    resetPlanningState: template.resetPlanningState,
  });
  return { task, created: true, assistantId: assistant.id };
}
