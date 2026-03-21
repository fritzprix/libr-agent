import { safeInvoke } from './core';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ScheduledTasksBackend');

export type ScheduledTaskTimezone = 'utc' | 'local';

/** Matches the Rust `ScheduledTaskDto` */
export interface ScheduledTask {
  id: string;
  name: string;
  cronExpression: string;
  scheduleTimezone: ScheduledTaskTimezone;
  assistantId: string;
  /** Supports @playbook:name and @skill:name mention syntax */
  message: string;
  yoloMode: boolean;
  sessionId: string | null;
  workspaceOverride: string | null;
  enabled: boolean;
  lastRunAt: number | null;
  nextRunAt: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface CreateScheduledTaskRequest {
  name: string;
  cronExpression: string;
  scheduleTimezone?: ScheduledTaskTimezone;
  assistantId: string;
  message: string;
  yoloMode: boolean;
  workspaceOverride?: string | null;
}

export interface UpdateScheduledTaskRequest {
  name?: string;
  cronExpression?: string;
  scheduleTimezone?: ScheduledTaskTimezone;
  assistantId?: string;
  message?: string;
  yoloMode?: boolean;
  workspaceOverride?: string | null;
  enabled?: boolean;
}

export async function createScheduledTask(
  request: CreateScheduledTaskRequest,
): Promise<ScheduledTask> {
  logger.info('Creating scheduled task', { name: request.name });
  return safeInvoke<ScheduledTask>('create_scheduled_task', { request });
}

export async function listScheduledTasks(
  assistantId?: string,
): Promise<ScheduledTask[]> {
  return safeInvoke<ScheduledTask[]>('list_scheduled_tasks', { assistantId });
}

export async function getScheduledTask(
  id: string,
): Promise<ScheduledTask | null> {
  return safeInvoke<ScheduledTask | null>('get_scheduled_task', { id });
}

export async function updateScheduledTask(
  id: string,
  request: UpdateScheduledTaskRequest,
): Promise<ScheduledTask> {
  logger.info('Updating scheduled task', { id });
  return safeInvoke<ScheduledTask>('update_scheduled_task', { id, request });
}

export async function toggleScheduledTask(
  id: string,
  enabled: boolean,
): Promise<ScheduledTask> {
  return safeInvoke<ScheduledTask>('toggle_scheduled_task', { id, enabled });
}

export async function deleteScheduledTask(id: string): Promise<void> {
  logger.info('Deleting scheduled task', { id });
  await safeInvoke<void>('delete_scheduled_task', { id });
}
