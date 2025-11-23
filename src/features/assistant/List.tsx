import {
  getNewAssistantTemplate,
  useAssistantContext,
} from '@/context/AssistantContext';
import { EditorProvider } from '@/context/EditorContext';
import type { Assistant } from '@/models/chat';
import { useCallback, useState, useEffect } from 'react';
import { Button } from '../../components/ui';
import { Input } from '../../components/ui/input';
import AssistantEditor from './AssistantEditor';
import AssistantCard from './Card';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AssistantList');

export default function AssistantList() {
  const {
    assistants,
    saveAssistant,
    searchAssistants,
    setPaginationMode,
    currentPage,
    setPage,
    pageSize,
    totalAssistants,
  } = useAssistantContext();

  const [createNew, setCreateNew] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Assistant[] | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  // Enable paginated mode on mount
  useEffect(() => {
    setPaginationMode('paginated');
    return () => {
      // Reset to full mode when unmounting (for other components)
      setPaginationMode('full');
    };
  }, [setPaginationMode]);

  const handleCreateComplete = useCallback(
    (assistant: Assistant) => {
      saveAssistant(assistant);
    },
    [saveAssistant],
  );

  const handleSearch = useCallback(
    async (query: string) => {
      setSearchQuery(query);
      if (!query.trim()) {
        setSearchResults(null);
        return;
      }

      setIsSearching(true);
      try {
        const results = await searchAssistants(query);
        setSearchResults(results);
        logger.debug('Search results:', { query, count: results.length });
      } catch (error) {
        logger.error('Search failed', error);
        setSearchResults([]);
      } finally {
        setIsSearching(false);
      }
    },
    [searchAssistants],
  );

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
    setSearchResults(null);
  }, []);

  const displayedAssistants = searchResults ?? assistants;
  const totalPages = Math.ceil(totalAssistants / pageSize);
  const showPagination = !searchResults && totalPages > 1;

  return (
    <div className="w-full border-r border-muted flex flex-col h-full">
      {/* Button - Fixed at top */}
      <div className="p-4 border-b border-muted flex-shrink-0">
        <Button
          variant="default"
          className="w-full"
          onClick={() => setCreateNew(true)}
        >
          + 새 어시스턴트 만들기
        </Button>
      </div>

      {/* Search bar */}
      <div className="p-4 border-b border-muted flex-shrink-0">
        <div className="relative">
          <Input
            type="text"
            placeholder="Search assistants..."
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            className="pr-20"
          />
          {searchQuery && (
            <Button
              variant="ghost"
              size="sm"
              className="absolute right-1 top-1/2 -translate-y-1/2 h-7"
              onClick={handleClearSearch}
            >
              Clear
            </Button>
          )}
        </div>
        {isSearching && (
          <div className="text-sm text-muted-foreground mt-2">Searching...</div>
        )}
        {searchResults !== null && !isSearching && (
          <div className="text-sm text-muted-foreground mt-2">
            Found {searchResults.length} results for &quot;{searchQuery}&quot;
          </div>
        )}
      </div>

      {/* Scrollable assistants list */}
      <div className="flex-1 overflow-y-auto p-4">
        {displayedAssistants.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">
            {searchResults !== null
              ? 'No assistants found matching your search'
              : 'No assistants available'}
          </div>
        ) : (
          <div className="space-y-2">
            {displayedAssistants.map((assistant) => (
              <AssistantCard key={assistant.id} assistant={assistant} />
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
              Previous
            </Button>
            <div className="text-sm text-muted-foreground">
              Page {currentPage} of {totalPages}
              <span className="ml-2">
                ({assistants.length} of {totalAssistants} assistants)
              </span>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage(currentPage + 1)}
              disabled={currentPage >= totalPages}
            >
              Next
            </Button>
          </div>
        </div>
      )}

      <EditorProvider
        initialValue={getNewAssistantTemplate()}
        onFinalize={handleCreateComplete}
      >
        <AssistantEditor.Dialog open={createNew} onOpenChange={setCreateNew} />
      </EditorProvider>
    </div>
  );
}
