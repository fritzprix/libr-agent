import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  DropdownMenuCheckboxItem,
} from '@/components/ui/dropdown-menu';
import { Calendar, User, Bookmark, SlidersHorizontal } from 'lucide-react';

import type { PlaybookSortState } from './types';

interface SortControlsProps {
  sortState: PlaybookSortState;
  setSortState: (
    state:
      | PlaybookSortState
      | ((prev: PlaybookSortState) => PlaybookSortState),
  ) => void;
}

export function SortControls({
  sortState,
  setSortState,
}: SortControlsProps) {
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
          <DropdownMenuCheckboxItem
            checked={sortState.sortMode === 'created_at'}
            onCheckedChange={() => setSortState((s) => ({ ...s, sortMode: 'created_at' }))}
          >
            <Calendar className="mr-2 h-4 w-4" />{' '}
            {t('playbook.sort.dateCreated')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortState.sortMode === 'assistant'}
            onCheckedChange={() => setSortState((s) => ({ ...s, sortMode: 'assistant' }))}
          >
            <User className="mr-2 h-4 w-4" /> {t('playbook.sort.assistant')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.order')}</DropdownMenuLabel>
          <DropdownMenuCheckboxItem
            checked={sortState.sortOrder === 'desc'}
            onCheckedChange={() => setSortState((s) => ({ ...s, sortOrder: 'desc' }))}
          >
            {t('playbook.sort.newestFirst')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortState.sortOrder === 'asc'}
            onCheckedChange={() => setSortState((s) => ({ ...s, sortOrder: 'asc' }))}
          >
            {t('playbook.sort.oldestFirst')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.grouping')}</DropdownMenuLabel>
          <DropdownMenuCheckboxItem
            checked={sortState.groupMode === 'none'}
            onCheckedChange={() => setSortState((s) => ({ ...s, groupMode: 'none' }))}
          >
            {t('playbook.sort.none')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortState.groupMode === 'time'}
            onCheckedChange={() => setSortState((s) => ({ ...s, groupMode: 'time' }))}
          >
            {t('playbook.sort.byTime')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortState.groupMode === 'assistant'}
            onCheckedChange={() => setSortState((s) => ({ ...s, groupMode: 'assistant' }))}
          >
            {t('playbook.sort.byAssistant')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem
            checked={sortState.bookmarkFirst}
            onCheckedChange={() => setSortState((s) => ({ ...s, bookmarkFirst: !s.bookmarkFirst }))}
          >
            <Bookmark className="mr-2 h-4 w-4" />{' '}
            {t('playbook.sort.bookmarksFirst')}
          </DropdownMenuCheckboxItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
