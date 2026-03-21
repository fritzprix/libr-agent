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
    <div className="p-8 h-full flex flex-col bg-background/50">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full gap-8">
        {/* Header Section */}
        <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-6 animate-in fade-in slide-in-from-top-4 duration-700">
          <div className="flex items-center gap-5">
            <div className="flex items-center justify-center w-14 h-14 bg-primary/10 text-primary rounded-2xl shadow-sm ring-1 ring-primary/20">
              <Users size={28} />
            </div>
            <div>
              <h1 className="text-3xl font-bold tracking-tight text-foreground">
                {t('assistant.list.title')}
              </h1>
              <p className="text-sm text-muted-foreground mt-1 font-sans">
                {t('assistant.list.subtitle')}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 w-full sm:w-auto">
            <div className="relative flex-1 sm:w-72 group">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground transition-colors group-focus-within:text-primary" />
              <Input
                type="text"
                placeholder={t('assistant.list.searchPlaceholder')}
                value={searchQuery}
                onChange={(e) => handleSearch(e.target.value)}
                className="pl-10 pr-10 h-10 bg-background/50 border-border/50 focus:border-primary/50 transition-all rounded-xl shadow-sm"
                aria-label={t('assistant.list.searchAriaLabel')}
              />
              {searchQuery && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute right-1 top-1/2 -translate-y-1/2 h-8 w-8 text-muted-foreground hover:text-foreground"
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
              className="h-10 px-5 whitespace-nowrap shadow-lg transition-all active:scale-95 rounded-xl font-bold"
            >
              <Plus size={18} className="mr-2" />
              {t('assistant.list.create')}
            </Button>
          </div>
        </div>

        {isSearching && (
          <div className="text-xs font-bold uppercase tracking-widest text-primary/60 animate-pulse font-sans ml-1">
            {t('assistant.list.searching')}
          </div>
        )}
        {searchResults !== null && !isSearching && (
          <div className="text-xs font-bold uppercase tracking-widest text-muted-foreground/60 font-sans ml-1">
            {t('assistant.list.searchResults', {
              count: searchResults.length,
              query: searchQuery,
            })}
          </div>
        )}

        {/* Scrollable assistants list */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-8 no-scrollbar animate-in fade-in slide-in-from-bottom-4 duration-700 delay-150">
          {displayedAssistants.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-center p-12 border border-dashed rounded-[2rem] bg-muted/10">
              <Users className="w-12 h-12 text-muted-foreground/20 mb-4" />
              <p className="text-muted-foreground font-sans max-w-xs">
                {searchResults !== null
                  ? t('assistant.list.noResults')
                  : t('assistant.list.empty')}
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4">
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
