import type { Playbook } from '@/types/playbook';

export type SortMode = 'created_at' | 'assistant';
export type SortOrder = 'asc' | 'desc';
export type GroupMode = 'none' | 'time' | 'assistant';

export interface PlaybookSortState {
  sortMode: SortMode;
  sortOrder: SortOrder;
  groupMode: GroupMode;
  bookmarkFirst: boolean;
}

export type PlaybookWithMeta = Playbook & {
  id: string;
  createdAt: Date;
  updatedAt: Date;
};
