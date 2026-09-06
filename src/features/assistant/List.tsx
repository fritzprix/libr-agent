import {
  getNewAssistantTemplate,
  useAssistantContext,
} from '@/context/AssistantContext';
import { EditorProvider } from '@/context/EditorContext';
import type { Assistant } from '@/models/chat';
import { useCallback, useEffect, useMemo, useRef } from 'react';
import { Button } from '../../components/ui';
import { Input } from '../../components/ui/input';
import AssistantEditor from './AssistantEditor';
import AssistantCard from './Card';
import { useTranslation } from 'react-i18next';
import { Search, X, Users, Plus, Loader2 } from 'lucide-react';
import { useAssistantsList } from './hooks/useAssistantsList';

export default function AssistantList() {
  const {
    assistants,
    saveAssistant,
    totalAssistants,
    loading,
    isLoadingMore,
    hasMore,
    loadMore,
  } = useAssistantContext();
  const { t } = useTranslation('common');
  const listScrollRef = useRef<HTMLDivElement>(null);
  const loadMoreSentinelRef = useRef<HTMLDivElement>(null);

  const {
    createNew,
    setCreateNew,
    searchQuery,
    searchResults,
    displayedAssistants,
    isSearching,
    expandedId,
    exitingIds,
    builtinToolsMap,
    mcpServersMap,
    handleToggleExpand,
    handleSearch,
    handleClearSearch,
    handleDeleteAssistant,
  } = useAssistantsList();

  const createInitialValue = useMemo(() => {
    if (!createNew) return null;
    return getNewAssistantTemplate();
  }, [createNew]);

  const preserveListScroll = useCallback(() => {
    const el = listScrollRef.current;
    if (!el) return;
    const top = el.scrollTop;
    const restore = () => {
      el.scrollTop = top;
    };
    restore();
    requestAnimationFrame(restore);
  }, []);

  const handleCreateComplete = useCallback(
    (assistant: Assistant) => {
      saveAssistant(assistant);
    },
    [saveAssistant],
  );

  const handleDelete = useCallback(
    async (assistantId: string) => {
      await handleDeleteAssistant(assistantId, {
        preserveScroll: preserveListScroll,
      });
    },
    [handleDeleteAssistant, preserveListScroll],
  );

  const isSearchActive = searchResults !== null;
  const canLoadMore = hasMore && !isSearchActive && !loading && !isLoadingMore;

  useEffect(() => {
    if (!canLoadMore) return;

    const root = listScrollRef.current;
    const sentinel = loadMoreSentinelRef.current;
    if (!root || !sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          void loadMore();
        }
      },
      { root, rootMargin: '240px 0px', threshold: 0 },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [canLoadMore, loadMore, displayedAssistants.length]);

  const showInitialLoading =
    loading && assistants.length === 0 && !searchResults;

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
              count: displayedAssistants.length,
              query: searchQuery,
            })}
          </div>
        )}

        {/* Scrollable assistants list */}
        <div
          ref={listScrollRef}
          className="flex-1 min-h-0 overflow-y-auto pr-2 pb-8 no-scrollbar animate-in fade-in slide-in-from-bottom-4 duration-700 delay-150"
        >
          {showInitialLoading ? (
            <div
              className="h-full flex flex-col items-center justify-center text-center p-12 gap-3"
              role="status"
              aria-live="polite"
            >
              <Loader2 className="w-8 h-8 text-primary/60 animate-spin" />
              <p className="text-muted-foreground font-sans text-sm">
                {t('assistant.list.loading')}
              </p>
            </div>
          ) : displayedAssistants.length === 0 ? (
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
                  isExiting={exitingIds.has(assistant.id)}
                  onToggle={() => handleToggleExpand(assistant.id)}
                  onDelete={handleDelete}
                  builtinToolsMap={builtinToolsMap}
                  mcpServersMap={mcpServersMap}
                />
              ))}

              {!isSearchActive && hasMore ? (
                <div
                  ref={loadMoreSentinelRef}
                  className="flex items-center justify-center gap-2 py-4"
                  role="status"
                  aria-live="polite"
                >
                  {isLoadingMore ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin text-primary/60" />
                      <span className="text-sm text-muted-foreground font-sans">
                        {t('assistant.list.loadingMore')}
                      </span>
                    </>
                  ) : (
                    <span className="text-xs text-muted-foreground/60 font-sans">
                      {t('assistant.list.totalInfo', {
                        count: assistants.length,
                        total: totalAssistants,
                      })}
                    </span>
                  )}
                </div>
              ) : null}
            </div>
          )}
        </div>

        {createNew && createInitialValue ? (
          <EditorProvider
            initialValue={createInitialValue}
            onFinalize={handleCreateComplete}
          >
            <AssistantEditor.Dialog
              open={createNew}
              onOpenChange={setCreateNew}
            />
          </EditorProvider>
        ) : null}
      </div>
    </div>
  );
}
