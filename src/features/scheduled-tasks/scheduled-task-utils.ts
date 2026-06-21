import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';

export function compareScheduledTasks(
  left: ScheduledTask,
  right: ScheduledTask,
) {
  if (left.enabled !== right.enabled) {
    return left.enabled ? -1 : 1;
  }

  if (left.nextRunAt === null && right.nextRunAt !== null) {
    return 1;
  }
  if (left.nextRunAt !== null && right.nextRunAt === null) {
    return -1;
  }
  if (left.nextRunAt !== null && right.nextRunAt !== null) {
    return left.nextRunAt - right.nextRunAt;
  }

  return left.name.localeCompare(right.name);
}
