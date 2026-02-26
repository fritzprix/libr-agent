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

export type SortMode = 'created_at' | 'assistant';
export type SortOrder = 'asc' | 'desc';
export type GroupMode = 'none' | 'time' | 'assistant';

interface SortControlsProps {
  sortMode: SortMode;
  setSortMode: (mode: SortMode) => void;
  sortOrder: SortOrder;
  setSortOrder: (order: SortOrder) => void;
  groupMode: GroupMode;
  setGroupMode: (mode: GroupMode) => void;
  bookmarkFirst: boolean;
  onBookmarkFirstToggle: () => void;
}

export function SortControls({
  sortMode,
  setSortMode,
  sortOrder,
  setSortOrder,
  groupMode,
  setGroupMode,
  bookmarkFirst,
  onBookmarkFirstToggle,
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
            checked={sortMode === 'created_at'}
            onCheckedChange={() => setSortMode('created_at')}
          >
            <Calendar className="mr-2 h-4 w-4" />{' '}
            {t('playbook.sort.dateCreated')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortMode === 'assistant'}
            onCheckedChange={() => setSortMode('assistant')}
          >
            <User className="mr-2 h-4 w-4" /> {t('playbook.sort.assistant')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.order')}</DropdownMenuLabel>
          <DropdownMenuCheckboxItem
            checked={sortOrder === 'desc'}
            onCheckedChange={() => setSortOrder('desc')}
          >
            {t('playbook.sort.newestFirst')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={sortOrder === 'asc'}
            onCheckedChange={() => setSortOrder('asc')}
          >
            {t('playbook.sort.oldestFirst')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t('playbook.sort.grouping')}</DropdownMenuLabel>
          <DropdownMenuCheckboxItem
            checked={groupMode === 'none'}
            onCheckedChange={() => setGroupMode('none')}
          >
            {t('playbook.sort.none')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={groupMode === 'time'}
            onCheckedChange={() => setGroupMode('time')}
          >
            {t('playbook.sort.byTime')}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={groupMode === 'assistant'}
            onCheckedChange={() => setGroupMode('assistant')}
          >
            {t('playbook.sort.byAssistant')}
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem
            checked={bookmarkFirst}
            onCheckedChange={onBookmarkFirstToggle}
          >
            <Bookmark className="mr-2 h-4 w-4" />{' '}
            {t('playbook.sort.bookmarksFirst')}
          </DropdownMenuCheckboxItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
