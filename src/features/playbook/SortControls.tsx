import { Dispatch, SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from '@/components/ui/dropdown-menu';
import { Calendar, User, Bookmark, SlidersHorizontal } from 'lucide-react';

import type {
  PlaybookSortState,
  SortMode,
  SortOrder,
  GroupMode,
} from './types';

const SORT_MODES: SortMode[] = ['created_at', 'assistant'];
const SORT_ORDERS: SortOrder[] = ['asc', 'desc'];
const GROUP_MODES: GroupMode[] = ['none', 'time', 'assistant'];

function toSortMode(v: string): SortMode {
  return SORT_MODES.includes(v as SortMode) ? (v as SortMode) : 'created_at';
}
function toSortOrder(v: string): SortOrder {
  return SORT_ORDERS.includes(v as SortOrder) ? (v as SortOrder) : 'desc';
}
function toGroupMode(v: string): GroupMode {
  return GROUP_MODES.includes(v as GroupMode) ? (v as GroupMode) : 'none';
}

interface SortControlsProps {
  sortState: PlaybookSortState;
  setSortState: Dispatch<SetStateAction<PlaybookSortState>>;
}

export function SortControls({ sortState, setSortState }: SortControlsProps) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-2">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm" className="h-8 gap-2">
            <SlidersHorizontal className="h-4 w-4" />
            <span>{t('playbook.sort.display')}</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuLabel>{t('playbook.sort.sortBy')}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={sortState.sortMode}
            onValueChange={(v) =>
              setSortState((s) => ({ ...s, sortMode: toSortMode(v) }))
            }
          >
            <DropdownMenuRadioItem value="created_at">
              <Calendar className="mr-2 h-4 w-4" />{' '}
              {t('playbook.sort.dateCreated')}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="assistant">
              <User className="mr-2 h-4 w-4" /> {t('playbook.sort.assistant')}
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.order')}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={sortState.sortOrder}
            onValueChange={(v) =>
              setSortState((s) => ({ ...s, sortOrder: toSortOrder(v) }))
            }
          >
            <DropdownMenuRadioItem value="desc">
              {t('playbook.sort.newestFirst')}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="asc">
              {t('playbook.sort.oldestFirst')}
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.grouping')}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={sortState.groupMode}
            onValueChange={(v) =>
              setSortState((s) => ({ ...s, groupMode: toGroupMode(v) }))
            }
          >
            <DropdownMenuRadioItem value="none">
              {t('playbook.sort.none')}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="time">
              {t('playbook.sort.byTime')}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="assistant">
              {t('playbook.sort.byAssistant')}
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>

          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem
            checked={sortState.bookmarkFirst}
            onCheckedChange={() =>
              setSortState((s) => ({ ...s, bookmarkFirst: !s.bookmarkFirst }))
            }
          >
            <Bookmark className="mr-2 h-4 w-4" />{' '}
            {t('playbook.sort.bookmarksFirst')}
          </DropdownMenuCheckboxItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
