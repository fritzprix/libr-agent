import type { Playbook } from '@/types/playbook';

export type PlaybookWithMeta = Playbook & {
  id: string;
  createdAt: Date;
};

export function groupPlaybooksByTime(
  playbooks: PlaybookWithMeta[],
): Record<string, PlaybookWithMeta[]> {
  const groups: Record<string, PlaybookWithMeta[]> = {
    Today: [],
    Yesterday: [],
    'This Week': [],
    'Last Month': [],
    Older: [],
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
      groups['Today'].push(pb);
    } else if (diffDays === 1) {
      groups['Yesterday'].push(pb);
    } else if (diffDays <= 7) {
      groups['This Week'].push(pb);
    } else if (diffDays <= 30) {
      groups['Last Month'].push(pb);
    } else {
      groups['Older'].push(pb);
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
    const name = assistantMap[pb.agentId]?.name || 'Unknown Assistant';
    if (!groups[name]) {
      groups[name] = [];
    }
    groups[name].push(pb);
  });

  return groups;
}

export function getGroupOrder(mode: 'time' | 'assistant'): string[] {
  if (mode === 'time') {
    return ['Today', 'Yesterday', 'This Week', 'Last Month', 'Older'];
  }
  return []; // Consumer should sort keys alphabetically if empty array returned
}
