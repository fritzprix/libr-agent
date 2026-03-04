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
              setSortState((s) => ({ ...s, sortMode: v as SortMode }))
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
              setSortState((s) => ({ ...s, sortOrder: v as SortOrder }))
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
              setSortState((s) => ({ ...s, groupMode: v as GroupMode }))
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
