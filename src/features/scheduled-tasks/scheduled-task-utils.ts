import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';

export interface ScheduledTaskGroupSection {
  key: string;
  groupId: string | null;
  groupName: string;
  tasks: ScheduledTask[];
  enabledCount: number;
}

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

export function buildScheduledTaskGroups(
  tasks: ScheduledTask[],
): ScheduledTaskGroupSection[] {
  const groups = new Map<string, ScheduledTaskGroupSection>();

  for (const task of tasks) {
    if (!task.groupName) {
      continue;
    }

    const key = task.groupId ?? `group:${task.groupName}`;
    const existing = groups.get(key);
    if (existing) {
      existing.tasks.push(task);
      continue;
    }

    groups.set(key, {
      key,
      groupId: task.groupId,
      groupName: task.groupName,
      tasks: [task],
      enabledCount: 0,
    });
  }

  return Array.from(groups.values())
    .map((group) => {
      const sortedTasks = [...group.tasks].sort(compareScheduledTasks);
      return {
        ...group,
        tasks: sortedTasks,
        enabledCount: sortedTasks.reduce(
          (count, task) => (task.enabled ? count + 1 : count),
          0,
        ),
      };
    })
    .sort((left, right) => left.groupName.localeCompare(right.groupName));
}
