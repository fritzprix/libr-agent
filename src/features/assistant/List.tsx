import {
  getNewAssistantTemplate,
  useAssistantContext,
} from '@/context/AssistantContext';
import { EditorProvider } from '@/context/EditorContext';
import type { Assistant } from '@/models/chat';
import { useCallback } from 'react';
import { Button } from '../../components/ui';
import { Input } from '../../components/ui/input';
import AssistantEditor from './AssistantEditor';
import AssistantCard from './Card';
import { useTranslation } from 'react-i18next';
import { Search, X, Users, Plus } from 'lucide-react';
import { useAssistantsList } from './hooks/useAssistantsList';

export default function AssistantList() {
  const {
    assistants,
    saveAssistant,
    currentPage,
    setPage,
    pageSize,
    totalAssistants,
  } = useAssistantContext();
  const { t } = useTranslation('common');

  const {
    createNew,
    setCreateNew,
    searchQuery,
    searchResults,
    isSearching,
    expandedId,
    builtinToolsMap,
    mcpServersMap,
    handleToggleExpand,
    handleSearch,
    handleClearSearch,
  } = useAssistantsList();

  const handleCreateComplete = useCallback(
    (assistant: Assistant) => {
      saveAssistant(assistant);
    },
    [saveAssistant],
  );

  const displayedAssistants = searchResults ?? assistants;
  const totalPages = Math.ceil(totalAssistants / pageSize);
  const showPagination = !searchResults && totalPages > 1;

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 mb-8">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
              <Users size={28} />
            </div>
            <div>
              <h1 className="text-2xl text-foreground font-semibold tracking-tight">
                {t('assistant.list.title')}
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                {t('assistant.list.subtitle')}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 w-full sm:w-auto">
            <div className="relative flex-1 sm:w-64">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <Input
                type="text"
                placeholder={t('assistant.list.searchPlaceholder')}
                value={searchQuery}
                onChange={(e) => handleSearch(e.target.value)}
                className="pl-9 pr-9 h-9"
                aria-label={t('assistant.list.searchAriaLabel')}
              />
              {searchQuery && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute right-1 top-1/2 -translate-y-1/2 h-7 w-7"
                  onClick={handleClearSearch}
                  aria-label={t('assistant.list.clearSearchAriaLabel')}
                >
                  <X className="h-4 w-4" />
                </Button>
              )}
            </div>
            <Button
              variant="default"
              onClick={() => setCreateNew(true)}
              className="h-9 whitespace-nowrap"
            >
              <Plus size={16} className="mr-2" />
              {t('assistant.list.create')}
            </Button>
          </div>
        </div>

        {isSearching && (
          <div className="text-sm text-muted-foreground mb-4">
            {t('assistant.list.searching')}
          </div>
        )}
        {searchResults !== null && !isSearching && (
          <div className="text-sm text-muted-foreground mb-4">
            {t('assistant.list.searchResults', {
              count: searchResults.length,
              query: searchQuery,
            })}
          </div>
        )}

        {/* Scrollable assistants list */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          {displayedAssistants.length === 0 ? (
            <div className="text-center text-muted-foreground py-8">
              {searchResults !== null
                ? t('assistant.list.noResults')
                : t('assistant.list.empty')}
            </div>
          ) : (
            <div className="space-y-2">
              {displayedAssistants.map((assistant) => (
                <AssistantCard
                  key={assistant.id}
                  assistant={assistant}
                  isExpanded={expandedId === assistant.id}
                  onToggle={() => handleToggleExpand(assistant.id)}
                  builtinToolsMap={builtinToolsMap}
                  mcpServersMap={mcpServersMap}
                />
              ))}
            </div>
          )}
        </div>

        {/* Pagination controls */}
        {showPagination && (
          <div className="p-4 border-t border-muted flex-shrink-0">
            <div className="flex items-center justify-between gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(currentPage - 1)}
                disabled={currentPage === 1}
              >
                {t('assistant.list.pagination.previous')}
              </Button>
              <div className="text-sm text-muted-foreground">
                {t('assistant.list.pagination.pageInfo', {
                  current: currentPage,
                  total: totalPages,
                })}
                <span className="ml-2">
                  {t('assistant.list.pagination.totalInfo', {
                    count: assistants.length,
                    total: totalAssistants,
                  })}
                </span>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(currentPage + 1)}
                disabled={currentPage >= totalPages}
              >
                {t('assistant.list.pagination.next')}
              </Button>
            </div>
          </div>
        )}

        <EditorProvider
          initialValue={getNewAssistantTemplate()}
          onFinalize={handleCreateComplete}
        >
          <AssistantEditor.Dialog
            open={createNew}
            onOpenChange={setCreateNew}
          />
        </EditorProvider>
      </div>
    </div>
  );
}
