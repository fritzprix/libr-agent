import type { PlaybookWithMeta } from './types';

// Keys must match common.json playbook.group keys
const GROUP_KEYS = {
  TODAY: 'playbook.group.today',
  YESTERDAY: 'playbook.group.yesterday',
  THIS_WEEK: 'playbook.group.thisWeek',
  LAST_MONTH: 'playbook.group.lastMonth',
  OLDER: 'playbook.group.older',
  UNKNOWN_ASSISTANT: 'playbook.group.unknownAssistant',
};

export function groupPlaybooksByTime(
  playbooks: PlaybookWithMeta[],
): Record<string, PlaybookWithMeta[]> {
  const groups: Record<string, PlaybookWithMeta[]> = {
    [GROUP_KEYS.TODAY]: [],
    [GROUP_KEYS.YESTERDAY]: [],
    [GROUP_KEYS.THIS_WEEK]: [],
    [GROUP_KEYS.LAST_MONTH]: [],
    [GROUP_KEYS.OLDER]: [],
  };

  const now = new Date();
  now.setHours(0, 0, 0, 0);
  const oneDay = 24 * 60 * 60 * 1000;

  playbooks.forEach((pb) => {
    const date = new Date(pb.createdAt);
    date.setHours(0, 0, 0, 0); // Normalize to midnight
    const diffTime = now.getTime() - date.getTime();
    const diffDays = Math.ceil(diffTime / oneDay);

    if (diffDays <= 0) {
      // Future created? or same day
      groups[GROUP_KEYS.TODAY].push(pb);
    } else if (diffDays === 1) {
      groups[GROUP_KEYS.YESTERDAY].push(pb);
    } else if (diffDays <= 7) {
      groups[GROUP_KEYS.THIS_WEEK].push(pb);
    } else if (diffDays <= 30) {
      groups[GROUP_KEYS.LAST_MONTH].push(pb);
    } else {
      groups[GROUP_KEYS.OLDER].push(pb);
    }
  });

  // Remove empty groups
  Object.keys(groups).forEach((key) => {
    if (groups[key].length === 0) {
      delete groups[key];
    }
  });

  return groups;
}

export function groupPlaybooksByAssistant(
  playbooks: PlaybookWithMeta[],
  assistantMap: Record<string, { name: string }>,
): Record<string, PlaybookWithMeta[]> {
  const groups: Record<string, PlaybookWithMeta[]> = {};

  playbooks.forEach((pb) => {
    const name = assistantMap[pb.agentId]?.name || GROUP_KEYS.UNKNOWN_ASSISTANT;
    if (!groups[name]) {
      groups[name] = [];
    }
    groups[name].push(pb);
  });

  return groups;
}

export function getGroupOrder(mode: 'time' | 'assistant'): string[] {
  if (mode === 'time') {
    return [
      GROUP_KEYS.TODAY,
      GROUP_KEYS.YESTERDAY,
      GROUP_KEYS.THIS_WEEK,
      GROUP_KEYS.LAST_MONTH,
      GROUP_KEYS.OLDER,
    ];
  }
  return []; // Consumer should sort keys alphabetically if empty array returned
}
