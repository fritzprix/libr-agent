# Playbook Feature Implementation Plan

**Sprint**: W3-Jan-26  
**Estimated Effort**: 9 days  
**Priority**: High  
**Status**: Planning

---

## Executive Summary

Implement a comprehensive Playbook feature that allows users to browse saved workflows with advanced sorting and grouping capabilities, and automatically execute them by starting an agent session. This feature enables zero-click workflow execution with flexible organization options.

### Key Deliverables

1. Playbook list page with grid layout, search, and flexible sorting
2. Visual grouping by time periods or assistants with collapsible sections
3. Bookmarking system for favorite playbooks
4. Auto-execution integration in AgentChatView
5. Navigation and routing updates

---

## Phase 1: Foundation & Infrastructure (Day 1-2)

### Task 1.1: Database Schema Extension

**File**: `src-tauri/src/entity/playbook.rs`  
**Effort**: 2 hours

**Changes**:

```rust
// Add bookmark field to Model struct
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playbooks")]
pub struct Model {
    // ... existing fields ...
    pub is_bookmarked: bool, // NEW
    pub created_at: i64,
    pub updated_at: i64,
}
```

**Migration**:

```sql
ALTER TABLE playbooks ADD COLUMN is_bookmarked BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX idx_playbooks_bookmarked ON playbooks(is_bookmarked, created_at DESC);
```

**Verification**:

- [ ] Migration runs successfully
- [ ] Index created on `is_bookmarked`
- [ ] Existing playbooks have default `false` value

---

### Task 1.2: Backend API - Bookmark Toggle

**File**: `src-tauri/src/commands/playbook_commands.rs`  
**Effort**: 2 hours

**Implementation**:

```rust
#[tauri::command]
pub async fn toggle_playbook_bookmark(
    db: State<'_, DatabaseConnection>,
    playbook_id: String,
    bookmarked: bool,
) -> Result<(), String> {
    let db = db.inner();

    let playbook = playbook::Entity::find_by_id(playbook_id.clone())
        .one(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| "Playbook not found".to_string())?;

    let mut active_model: playbook::ActiveModel = playbook.into();
    active_model.is_bookmarked = Set(bookmarked);
    active_model.updated_at = Set(chrono::Utc::now().timestamp_millis());

    active_model.update(db).await
        .map_err(|e| format!("Failed to update bookmark: {}", e))?;

    Ok(())
}
```

**Frontend Wrapper** (`src/lib/backend/playbooks.ts`):

```typescript
export async function togglePlaybookBookmark(
  id: string,
  bookmarked: boolean,
): Promise<void> {
  await safeInvoke<void>('toggle_playbook_bookmark', {
    playbookId: id,
    bookmarked,
  });
}
```

**Verification**:

- [ ] Command registered in `src-tauri/src/main.rs`
- [ ] Frontend wrapper compiles without errors
- [ ] Manual test: Toggle bookmark via Tauri devtools

---

### Task 1.3: Update List Query with Flexible Sorting

**File**: `src-tauri/src/mcp/builtin/playbook/operations.rs`  
**Effort**: 2 hours

**Changes**:

```rust
// Update list_playbooks to support flexible sorting
pub async fn list_playbooks(
    db: &DatabaseConnection,
    assistant_id: &str,
    args: Value,
    render_ui_flag: bool,
) -> Result<MCPResult, String> {
    // Extract sort parameters from args
    let sort_by = args["sort_by"].as_str().unwrap_or("created_at");
    let sort_order = args["sort_order"].as_str().unwrap_or("desc");
    let bookmark_first = args["bookmark_first"].as_bool().unwrap_or(true);

    let mut query = PlaybookEntity::find()
        .filter(playbook::Column::AssistantId.eq(assistant_id));

    // Apply bookmark priority if enabled
    if bookmark_first {
        query = query.order_by_desc(playbook::Column::IsBookmarked);
    }

    // Apply primary sort
    match sort_by {
        "created_at" => {
            query = if sort_order == "asc" {
                query.order_by_asc(playbook::Column::CreatedAt)
            } else {
                query.order_by_desc(playbook::Column::CreatedAt)
            };
        }
        "assistant" => {
            query = if sort_order == "asc" {
                query.order_by_asc(playbook::Column::AssistantId)
            } else {
                query.order_by_desc(playbook::Column::AssistantId)
            };
            // Secondary sort by date within assistant groups
            query = query.order_by_desc(playbook::Column::CreatedAt);
        }
        _ => {
            query = query.order_by_desc(playbook::Column::CreatedAt);
        }
    }

    // ... rest of implementation ...
}
```

**Verification**:

- [ ] Sort by created_at (asc/desc) works correctly
- [ ] Sort by assistant groups playbooks by assistant
- [ ] bookmark_first flag controls bookmark priority
- [ ] Test all sort combinations

---

## Phase 2: Frontend - Playbook List UI (Day 3-5)

### Task 2.1: Create Feature Directory Structure

**Effort**: 30 minutes

**Actions**:

```bash
mkdir -p src/features/playbook
touch src/features/playbook/List.tsx
touch src/features/playbook/Card.tsx
touch src/features/playbook/PlaybookGroup.tsx
touch src/features/playbook/SortControls.tsx
touch src/features/playbook/README.md
```

**Documentation** (`src/features/playbook/README.md`):

- Component overview
- Data flow diagram
- API integration notes
- Grouping and sorting architecture
- Usage examples

---

### Task 2.2: Implement Playbook Card Component

**File**: `src/features/playbook/Card.tsx`  
**Effort**: 3 hours

**Implementation**:

```typescript
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Bookmark, Play, Trash2 } from 'lucide-react';
import type { Playbook } from '@/types/playbook';
import { togglePlaybookBookmark } from '@/lib/backend/playbooks';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';

interface PlaybookCardProps {
  playbook: Playbook & { id: string; createdAt: Date };
  onDelete: (id: string) => void;
  onBookmarkToggle: () => void;
}

export function PlaybookCard({
  playbook,
  onDelete,
  onBookmarkToggle
}: PlaybookCardProps) {
  const navigate = useNavigate();
  const [isBookmarking, setIsBookmarking] = useState(false);

  const handleStart = () => {
    navigate(`/agent?playbookId=${playbook.id}`);
  };

  const handleBookmarkToggle = async (e: React.MouseEvent) => {
    e.stopPropagation(); // Prevent card click
    setIsBookmarking(true);
    try {
      await togglePlaybookBookmark(playbook.id, !playbook.isBookmarked);
      onBookmarkToggle();
      toast.success(playbook.isBookmarked ? 'Bookmark removed' : 'Bookmarked');
    } catch (error) {
      toast.error('Failed to toggle bookmark');
    } finally {
      setIsBookmarking(false);
    }
  };

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm(`Delete playbook "${playbook.goal}"?`)) {
      onDelete(playbook.id);
    }
  };

  return (
    <div className={cn(
      "border rounded-lg p-4 hover:shadow-md transition-all",
      playbook.isBookmarked && "border-primary bg-primary/5"
    )}>
      <div className="flex items-start justify-between mb-3">
        <h3 className="font-semibold text-lg line-clamp-2">
          {playbook.goal}
        </h3>
        <Button
          variant="ghost"
          size="icon"
          onClick={handleBookmarkToggle}
          disabled={isBookmarking}
          className="flex-shrink-0"
        >
          <Bookmark
            className={cn(
              "h-4 w-4",
              playbook.isBookmarked && "fill-current text-primary"
            )}
          />
        </Button>
      </div>

      <div className="text-sm text-muted-foreground space-y-1 mb-4">
        <p>{playbook.workflow.length} steps</p>
        <p>Created {playbook.createdAt.toLocaleDateString()}</p>
      </div>

      <div className="flex gap-2">
        <Button
          onClick={handleStart}
          className="flex-1"
          size="sm"
        >
          <Play className="h-4 w-4 mr-2" />
          Start
        </Button>
        <Button
          variant="destructive"
          size="icon"
          onClick={handleDelete}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
```

**Features**:

- Bookmark toggle with visual indicator
- Start button navigating to agent with playbook ID
- Delete button with confirmation
- Visual distinction for bookmarked items
- Responsive layout

**Verification**:

- [ ] Card renders correctly with playbook data
- [ ] Bookmark toggle works and updates UI
- [ ] Start button navigates to correct URL
- [ ] Delete button shows confirmation
- [ ] Hover states work as expected

---

### Task 2.3: Implement Sort Controls Component

**File**: `src/features/playbook/SortControls.tsx`  
**Effort**: 2 hours

**Implementation**:

```typescript
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';
import { ArrowUpDown, Calendar, User, Bookmark } from 'lucide-react';

export type SortMode = 'created_at' | 'assistant';
export type SortOrder = 'asc' | 'desc';
export type GroupMode = 'none' | 'time' | 'assistant';

interface SortControlsProps {
  sortMode: SortMode;
  sortOrder: SortOrder;
  groupMode: GroupMode;
  bookmarkFirst: boolean;
  onSortModeChange: (mode: SortMode) => void;
  onSortOrderChange: (order: SortOrder) => void;
  onGroupModeChange: (mode: GroupMode) => void;
  onBookmarkFirstToggle: () => void;
}

export function SortControls({
  sortMode,
  sortOrder,
  groupMode,
  bookmarkFirst,
  onSortModeChange,
  onSortOrderChange,
  onGroupModeChange,
  onBookmarkFirstToggle,
}: SortControlsProps) {
  return (
    <div className="flex items-center gap-2">
      {/* Sort Mode Dropdown */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm">
            <ArrowUpDown className="h-4 w-4 mr-2" />
            Sort: {sortMode === 'created_at' ? 'Date' : 'Assistant'}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            onClick={() => onSortModeChange('created_at')}
            className={sortMode === 'created_at' ? 'bg-accent' : ''}
          >
            <Calendar className="h-4 w-4 mr-2" />
            By Date
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() => onSortModeChange('assistant')}
            className={sortMode === 'assistant' ? 'bg-accent' : ''}
          >
            <User className="h-4 w-4 mr-2" />
            By Assistant
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => onSortOrderChange(sortOrder === 'asc' ? 'desc' : 'asc')}>
            Order: {sortOrder === 'asc' ? '↑ Ascending' : '↓ Descending'}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {/* Group Mode Dropdown */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm">
            Group: {groupMode === 'none' ? 'None' : groupMode === 'time' ? 'Time' : 'Assistant'}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            onClick={() => onGroupModeChange('none')}
            className={groupMode === 'none' ? 'bg-accent' : ''}
          >
            No Grouping
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() => onGroupModeChange('time')}
            className={groupMode === 'time' ? 'bg-accent' : ''}
          >
            <Calendar className="h-4 w-4 mr-2" />
            By Time Period
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() => onGroupModeChange('assistant')}
            className={groupMode === 'assistant' ? 'bg-accent' : ''}
          >
            <User className="h-4 w-4 mr-2" />
            By Assistant
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {/* Bookmark First Toggle */}
      <Button
        variant={bookmarkFirst ? 'default' : 'outline'}
        size="sm"
        onClick={onBookmarkFirstToggle}
      >
        <Bookmark className="h-4 w-4 mr-2" />
        Bookmarks First
      </Button>
    </div>
  );
}
```

**Features**:

- Sort mode selector (Date/Assistant)
- Sort order toggle (Ascending/Descending)
- Group mode selector (None/Time/Assistant)
- Bookmark priority toggle
- Visual feedback for active state

**Verification**:

- [ ] All dropdowns open correctly
- [ ] Selection states update UI
- [ ] Active states show correct styling
- [ ] Callbacks fire with correct values

---

### Task 2.4: Implement Playbook Group Component

**File**: `src/features/playbook/PlaybookGroup.tsx`  
**Effort**: 2 hours

**Implementation**:

```typescript
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { PlaybookCard } from './Card';
import type { Playbook } from '@/types/playbook';
import { cn } from '@/lib/utils';

interface PlaybookGroupProps {
  title: string;
  playbooks: (Playbook & { id: string; createdAt: Date })[];
  defaultCollapsed?: boolean;
  onDelete: (id: string) => void;
  onBookmarkToggle: () => void;
}

export function PlaybookGroup({
  title,
  playbooks,
  defaultCollapsed = false,
  onDelete,
  onBookmarkToggle,
}: PlaybookGroupProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);

  return (
    <div className="mb-6">
      {/* Group Header */}
      <Button
        variant="ghost"
        onClick={() => setIsCollapsed(!isCollapsed)}
        className="w-full justify-start mb-3 hover:bg-accent/50 px-2"
      >
        {isCollapsed ? (
          <ChevronRight className="h-4 w-4 mr-2" />
        ) : (
          <ChevronDown className="h-4 w-4 mr-2" />
        )}
        <h2 className="text-sm font-semibold uppercase tracking-wide">
          {title} ({playbooks.length})
        </h2>
      </Button>

      {/* Group Content */}
      {!isCollapsed && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 pl-6">
          {playbooks.map((playbook) => (
            <PlaybookCard
              key={playbook.id}
              playbook={playbook}
              onDelete={onDelete}
              onBookmarkToggle={onBookmarkToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
```

**Features**:

- Collapsible group sections
- Item count badge in header
- Smooth expand/collapse animation
- Grid layout for cards within group
- Accessible keyboard navigation

**Verification**:

- [ ] Groups collapse/expand on click
- [ ] Count badge shows correct number
- [ ] Cards render correctly within groups
- [ ] Collapse state persists during interaction
- [ ] Keyboard navigation works (Space/Enter)

---

### Task 2.5: Implement Grouping Utilities

**File**: `src/features/playbook/grouping-utils.ts`  
**Effort**: 2 hours

**Implementation**:

```typescript
import type { Playbook } from '@/types/playbook';

export type PlaybookWithMeta = Playbook & { id: string; createdAt: Date };

/**
 * Group playbooks by time periods
 */
export function groupPlaybooksByTime(
  playbooks: PlaybookWithMeta[],
): Record<string, PlaybookWithMeta[]> {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const thisWeek = new Date(today);
  thisWeek.setDate(thisWeek.getDate() - 7);
  const lastMonth = new Date(today);
  lastMonth.setMonth(lastMonth.getMonth() - 1);

  const groups: Record<string, PlaybookWithMeta[]> = {
    Today: [],
    Yesterday: [],
    'This Week': [],
    'Last Month': [],
    Older: [],
  };

  playbooks.forEach((playbook) => {
    const createdAt = playbook.createdAt;

    if (createdAt >= today) {
      groups['Today'].push(playbook);
    } else if (createdAt >= yesterday) {
      groups['Yesterday'].push(playbook);
    } else if (createdAt >= thisWeek) {
      groups['This Week'].push(playbook);
    } else if (createdAt >= lastMonth) {
      groups['Last Month'].push(playbook);
    } else {
      groups['Older'].push(playbook);
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

/**
 * Group playbooks by assistant
 */
export function groupPlaybooksByAssistant(
  playbooks: PlaybookWithMeta[],
  assistantMap: Record<string, { name: string }>,
): Record<string, PlaybookWithMeta[]> {
  const groups: Record<string, PlaybookWithMeta[]> = {};

  playbooks.forEach((playbook) => {
    const assistantId = playbook.agentId;
    const assistantName =
      assistantMap[assistantId]?.name || 'Unknown Assistant';

    if (!groups[assistantName]) {
      groups[assistantName] = [];
    }
    groups[assistantName].push(playbook);
  });

  return groups;
}

/**
 * Get ordered group keys for consistent rendering
 */
export function getGroupOrder(groupMode: 'time' | 'assistant'): string[] {
  if (groupMode === 'time') {
    return ['Today', 'Yesterday', 'This Week', 'Last Month', 'Older'];
  }
  return []; // Assistant groups will be sorted alphabetically
}
```

**Features**:

- Time-based grouping with smart date calculations
- Assistant-based grouping with name lookup
- Empty group filtering
- Consistent group ordering
- Type-safe with full TypeScript support

**Verification**:

- [ ] Time groups calculate correctly across midnight boundary
- [ ] Assistant groups handle missing assistant names
- [ ] Empty groups are filtered out
- [ ] Playbooks are correctly assigned to groups

---

### Task 2.6: Implement Playbook List Page

**File**: `src/features/playbook/List.tsx`  
**Effort**: 4 hours

**Implementation**:

```typescript
import { useState, useEffect, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { PlaybookCard } from './Card';
import { listPlaybooks, deletePlaybook } from '@/lib/backend/playbooks';
import type { Playbook } from '@/types/playbook';
import { toast } from 'sonner';
import { Search, RefreshCw } from 'lucide-react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('PlaybookList');

export default function PlaybookList() {
  const [playbooks, setPlaybooks] = useState<
    (Playbook & { id: string; createdAt: Date })[]
  >([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  const loadPlaybooks = useCallback(async () => {
    setIsLoading(true);
    try {
      const data = await listPlaybooks();
      setPlaybooks(data);
      logger.info('Loaded playbooks', { count: data.length });
    } catch (error) {
      logger.error('Failed to load playbooks', error);
      toast.error('Failed to load playbooks');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPlaybooks();
  }, [loadPlaybooks]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await deletePlaybook(id);
      setPlaybooks((prev) => prev.filter((p) => p.id !== id));
      toast.success('Playbook deleted');
    } catch (error) {
      logger.error('Failed to delete playbook', error);
      toast.error('Failed to delete playbook');
    }
  }, []);

  const filteredPlaybooks = playbooks.filter((p) =>
    p.goal.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const bookmarkedPlaybooks = filteredPlaybooks.filter((p) => p.isBookmarked);
  const regularPlaybooks = filteredPlaybooks.filter((p) => !p.isBookmarked);

  return (
    <div className="h-full flex flex-col p-6">
      <div className="mb-6">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-2xl font-bold">Playbooks</h1>
          <Button
            variant="ghost"
            size="icon"
            onClick={loadPlaybooks}
            disabled={isLoading}
          >
            <RefreshCw className={cn("h-4 w-4", isLoading && "animate-spin")} />
          </Button>
        </div>

        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            type="text"
            placeholder="Search playbooks..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-10"
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : filteredPlaybooks.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <p className="text-muted-foreground">
              {searchQuery ? 'No playbooks found' : 'No playbooks yet'}
            </p>
          </div>
        ) : (
          <div className="space-y-6">
            {bookmarkedPlaybooks.length > 0 && (
              <div>
                <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase">
                  Bookmarked ({bookmarkedPlaybooks.length})
                </h2>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {bookmarkedPlaybooks.map((playbook) => (
                    <PlaybookCard
                      key={playbook.id}
                      playbook={playbook}
                      onDelete={handleDelete}
                      onBookmarkToggle={loadPlaybooks}
                    />
                  ))}
                </div>
              </div>
            )}

            {regularPlaybooks.length > 0 && (
              <div>
                <h2 className="text-sm font-semibold text-muted-foreground mb-3 uppercase">
                  All Playbooks ({regularPlaybooks.length})
                </h2>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {regularPlaybooks.map((playbook) => (
                    <PlaybookCard
                      key={playbook.id}
                      playbook={playbook}
                      onDelete={handleDelete}
                      onBookmarkToggle={loadPlaybooks}
                    />
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
```

**Features**:

- Grid layout with responsive columns
- Sort controls (Date/Assistant, Asc/Desc)
- Group mode selection (None/Time/Assistant)
- Visual grouping with collapsible sections
- Bookmark priority toggle
- Search functionality
- Refresh button
- Loading and empty states
- Error handling with toasts
- Memoized filtering and grouping for performance

**Verification**:

- [ ] Page renders without errors
- [ ] Playbooks load on mount with default sort
- [ ] Search filters correctly across all groups
- [ ] Sort controls update data correctly
- [ ] Group mode switches between none/time/assistant
- [ ] Time groups show correct date ranges
- [ ] Assistant groups show correct names
- [ ] Collapsible groups expand/collapse properly
- [ ] Bookmark first toggle works
- [ ] Empty state shows when no playbooks
- [ ] Loading spinner shows during fetch
- [ ] No re-renders during typing in search

---

## Phase 3: Navigation & Routing (Day 6)

### Task 3.1: Add Sidebar Navigation Link

**File**: `src/components/layout/AppSidebar.tsx`  
**Effort**: 30 minutes

**Changes**:

```typescript
import { BookOpen } from 'lucide-react';

// Add after History section (around line 105)
<SidebarGroup>
  <SidebarGroupLabel className="text-sm font-semibold uppercase tracking-wide mb-2">
    Workflows
  </SidebarGroupLabel>
  <SidebarGroupContent>
    <SidebarMenu>
      <SidebarMenuItem>
        <Link to="/playbooks">
          <SidebarMenuButton
            isActive={location.pathname === '/playbooks'}
            tooltip="Playbooks"
          >
            <BookOpen size={16} />
            <span>Playbooks</span>
          </SidebarMenuButton>
        </Link>
      </SidebarMenuItem>
    </SidebarMenu>
  </SidebarGroupContent>
</SidebarGroup>
```

**Verification**:

- [ ] Link appears in sidebar
- [ ] Active state works when on /playbooks route
- [ ] Tooltip shows on hover (collapsed state)
- [ ] Icon and text align correctly

---

### Task 3.2: Add Route to App.tsx

**File**: `src/app/App.tsx`  
**Effort**: 15 minutes

**Changes**:

```typescript
// Add import
const PlaybookList = lazy(() => import('@/features/playbook/List'));

// Add route (around line 70)
<Route
  path="/playbooks"
  element={<PlaybookList />}
/>
```

**Verification**:

- [ ] Route registers without errors
- [ ] Navigation to /playbooks loads page
- [ ] Lazy loading works (check network tab)
- [ ] Suspense fallback shows during load

---

### Task 3.3: Update Backend API Signature

**File**: `src/lib/backend/playbooks.ts`  
**Effort**: 30 minutes

**Changes**:

```typescript
export interface ListPlaybooksOptions {
  sort_by?: 'created_at' | 'assistant';
  sort_order?: 'asc' | 'desc';
  bookmark_first?: boolean;
}

export async function listPlaybooks(
  options: ListPlaybooksOptions = {},
): Promise<(Playbook & { id: string; createdAt: Date; updatedAt: Date })[]> {
  const dtos = await safeInvoke<PlaybookDto[]>('list_playbooks', {
    sortBy: options.sort_by || 'created_at',
    sortOrder: options.sort_order || 'desc',
    bookmarkFirst: options.bookmark_first !== false,
  });

  return dtos.map(deserializePlaybook);
}
```

**Verification**:

- [ ] Function signature accepts sort options
- [ ] Options are passed to backend correctly
- [ ] Default values work when options not provided
- [ ] Type checking passes

---

### Task 3.4: Update Playbook Type with Bookmark Field

**File**: `src/types/playbook.ts`  
**Effort**: 15 minutes

**Changes**:

```typescript
export interface Playbook {
  id?: string;
  agentId: string;
  goal: string;
  initialCommand: string;
  workflow: PlaybookStep[];
  successCriteria: {
    description: string;
    requiredArtifacts?: string[];
  };
  isBookmarked?: boolean; // NEW
}
```

**File**: `src/lib/backend/playbooks.ts`  
**Update deserialization**:

```typescript
function deserializePlaybook(
  dto: PlaybookDto,
): Playbook & { id: string; createdAt: Date; updatedAt: Date } {
  return {
    id: dto.id,
    agentId: dto.sessionId,
    goal: dto.goal,
    initialCommand: dto.initialCommand || '',
    workflow: safeParsePlaybookWorkflow(dto.workflow),
    successCriteria: safeParseSuccessCriteria(dto.successCriteria),
    isBookmarked: dto.isBookmarked || false, // NEW
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  };
}
```

**Verification**:

- [ ] Type compiles without errors
- [ ] Deserialization includes isBookmarked field
- [ ] Default value is false for missing field

---

## Phase 4: Auto-Execution Integration (Day 7)

### Task 4.1: Implement Auto-Execution in AgentChatView

**File**: `src/features/agent/AgentChatView.tsx`  
**Effort**: 4 hours

**Implementation**:

```typescript
import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { toast } from 'sonner';

function AgentChatInner() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { session, isSessionLoading } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const [isExecutingPlaybook, setIsExecutingPlaybook] = useState(false);

  const { showWorkspacePanel } = useAgentWorkspace();
  const { showPlanningPanel } = useAgentPlanning();

  // Auto-execute playbook on mount
  useEffect(() => {
    const playbookId = searchParams.get('playbookId');

    // Guard conditions
    if (!playbookId) return;
    if (!session) return;
    if (isSessionLoading) return;
    if (isExecutingPlaybook) return;

    logger.info('Auto-executing playbook', {
      playbookId,
      sessionId: session.id
    });

    executePlaybookSelection(playbookId);

    // Clear query param to prevent re-execution
    setSearchParams({}, { replace: true });
  }, [session, isSessionLoading, searchParams, isExecutingPlaybook]);

  const executePlaybookSelection = async (playbookId: string) => {
    setIsExecutingPlaybook(true);

    try {
      logger.info('Calling selectPlaybook tool', { playbookId });

      // Call backend MCP tool
      const result = await invoke<{
        content: Array<{ type: string; text: string }>;
        structured_content?: unknown;
        is_error?: boolean;
      }>('mcp_call_tool', {
        sessionId: session.id,
        toolName: 'builtin_playbook__selectPlaybook',
        args: { id: playbookId }
      });

      if (result.is_error) {
        throw new Error('Playbook not found or failed to load');
      }

      // Create tool message pair with actual result
      const toolCallId = createId();
      const [toolCallMsg, toolResultMsg] = createToolMessagePair(
        'builtin_playbook__selectPlaybook',
        { id: playbookId },
        result.content,
        toolCallId,
        session.id,
        undefined,
        session.assistant?.id,
        'ui' // Mark as UI-triggered
      );

      logger.info('Injecting playbook messages', {
        playbookId,
        sessionId: session.id,
        toolCallId,
      });

      // Inject messages and trigger workflow
      await injectMessages([toolCallMsg, toolResultMsg], true);

      toast.success('Playbook loaded successfully');
      logger.info('Playbook auto-execution completed', { playbookId });

    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      logger.error('Failed to auto-select playbook', {
        playbookId,
        error: errorMsg
      });
      toast.error(`Failed to load playbook: ${errorMsg}`);
    } finally {
      setIsExecutingPlaybook(false);
    }
  };

  // Show loading overlay during playbook execution
  if (isExecutingPlaybook) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <RefreshCw className="h-8 w-8 animate-spin mx-auto mb-4" />
          <p className="text-lg">Loading playbook...</p>
          <p className="text-sm text-muted-foreground">
            Setting up workflow execution
          </p>
        </div>
      </div>
    );
  }

  return (
    <>
      <TimeLocationSystemPrompt />
      <div className="h-full w-full max-h-[100vh] font-mono flex rounded-lg overflow-hidden shadow-2xl">
        {showWorkspacePanel && <AgentWorkspacePanel />}

        <div className="flex-1 flex flex-col min-h-0 min-w-0">
          <AgentChatHeader />
          <AgentChatStatusBar />
          <AgentChatMessages />
          <AgentChatAttachedFiles />
          <AgentChatInput />
        </div>

        {showPlanningPanel && <AgentPlanningPanel />}
      </div>
    </>
  );
}
```

**Key Features**:

- Query param detection on mount
- Guard conditions prevent multiple executions
- Backend tool call with error handling
- Tool message pair creation
- Workflow injection with trigger
- Loading overlay during execution
- Query param cleanup after execution

**Verification**:

- [ ] Playbook loads automatically when URL has ?playbookId
- [ ] Loading overlay shows during execution
- [ ] Query param is cleared after execution
- [ ] Page refresh doesn't re-trigger execution
- [ ] Error states show toast messages
- [ ] Agent receives playbook details and starts workflow

---

### Task 4.2: Update AgentChatStartView Navigation

**File**: `src/features/agent/AgentChatStartView.tsx`  
**Effort**: 1 hour

**Purpose**: Support creating sessions with playbook pre-loading

**Changes**:

```typescript
// Add playbookId to URL when navigating from playbook page
const handleNavigateWithPlaybook = useCallback(
  async (assistant: Assistant, playbookId: string) => {
    try {
      setIsCreating(true);
      const session = await createSession({ assistant });
      navigate(`/agent/${session.id}?playbookId=${playbookId}`);
    } catch (error) {
      logger.error('Failed to create session with playbook', error);
      toast.error('Failed to start agent session');
    } finally {
      setIsCreating(false);
    }
  },
  [createSession, navigate],
);
```

**Note**: This change is optional. The playbook list can navigate directly to `/agent?playbookId={id}` and let AgentChatStartView handle assistant selection.

---

## Phase 5: Testing & Polish (Day 8-9)

### Task 5.1: Manual Testing Checklist

#### Playbook List Page

- [ ] Navigate to /playbooks from sidebar
- [ ] Playbooks load and display in grid
- [ ] Search filters playbooks correctly
- [ ] Bookmark toggle updates UI immediately
- [ ] Bookmarked playbooks appear at top
- [ ] Delete button removes playbook
- [ ] Empty state shows when no playbooks
- [ ] Loading state shows during fetch
- [ ] Error toast shows on API failure

#### Sorting & Grouping Features

- [ ] Sort by Date (ascending/descending) works correctly
- [ ] Sort by Assistant groups playbooks by assistant
- [ ] Bookmark First toggle affects sort order
- [ ] Group by None shows flat grid layout
- [ ] Group by Time creates Today/Yesterday/This Week/Last Month/Older sections
- [ ] Group by Assistant creates sections per assistant
- [ ] Empty time periods are not displayed
- [ ] Group sections collapse/expand correctly
- [ ] Item counts in group headers are accurate
- [ ] Sort controls UI updates to reflect current state
- [ ] Changing sort/group triggers data reload
- [ ] Search works across all groups
- [ ] Collapsed groups persist during re-render

#### Auto-Execution Flow

- [ ] Click "Start" on playbook card
- [ ] Agent session creates successfully
- [ ] Loading overlay shows during playbook load
- [ ] Playbook details appear in chat
- [ ] Agent starts executing workflow automatically
- [ ] Query param is cleared after execution
- [ ] Page refresh doesn't re-trigger execution
- [ ] Invalid playbook ID shows error
- [ ] Network failure shows error toast

#### Integration Tests

- [ ] Sidebar navigation works
- [ ] Route is registered correctly
- [ ] Lazy loading works for PlaybookList
- [ ] Type checking passes (pnpm typecheck)
- [ ] Linting passes (pnpm lint)
- [ ] Build succeeds (pnpm build)

---

### Task 5.2: Error Scenarios Testing

Test these error conditions:

1. **Playbook Not Found**
   - URL: `/agent?playbookId=nonexistent`
   - Expected: Error toast, no workflow starts

2. **Database Connection Error**
   - Simulate DB failure
   - Expected: Error toast, graceful degradation

3. **Session Not Ready**
   - Navigate before session loads
   - Expected: Wait for session, then execute

4. **Tool Execution Failure**
   - Mock tool failure
   - Expected: Error toast, no workflow starts

5. **Network Timeout**
   - Simulate network delay
   - Expected: Loading state, eventual timeout error

---

### Task 5.3: Performance Optimization

#### Lazy Loading

- [ ] PlaybookList is code-split
- [ ] Initial bundle size is acceptable
- [ ] Loading spinner shows during chunk load

#### Memoization

- [ ] Filtered playbooks use useMemo
- [ ] Event handlers use useCallback
- [ ] Expensive computations are memoized

#### API Optimization

- [ ] List query uses indexed columns
- [ ] Pagination is implemented (if needed)
- [ ] Debounce search input (300ms)

---

### Task 5.4: Code Quality

#### TypeScript

- [ ] No `any` types used
- [ ] All props interfaces defined
- [ ] Proper type guards for unknown data
- [ ] Error types are specific

#### React Best Practices

- [ ] Proper dependency arrays in hooks
- [ ] No unnecessary re-renders
- [ ] Cleanup functions in useEffect
- [ ] Proper key props in lists

#### Accessibility

- [ ] Semantic HTML elements
- [ ] ARIA labels where needed
- [ ] Keyboard navigation works
- [ ] Focus management is correct

---

## Phase 6: Documentation & Cleanup

### Task 6.1: Update Documentation

**Files to Update**:

1. `src/features/playbook/README.md` - Feature documentation
2. `docs/features/playbook-feature.md` - Architecture and design
3. `CHANGELOG.md` - Add feature entry
4. `README.md` - Update feature list

**Documentation Content**:

- Feature overview
- Component hierarchy
- Data flow diagrams
- API reference
- Usage examples
- Troubleshooting guide

---

### Task 6.2: Code Cleanup

**Actions**:

- [ ] Remove console.log statements
- [ ] Remove commented code
- [ ] Update import statements (no unused)
- [ ] Run `pnpm dead-code` and clean up
- [ ] Run `pnpm refactor:validate`
- [ ] Format with Prettier
- [ ] Fix any linting warnings

---

## Risk Mitigation

### Technical Risks

| Risk                                | Impact | Probability | Mitigation                              |
| ----------------------------------- | ------ | ----------- | --------------------------------------- |
| Query param re-execution on refresh | High   | Medium      | Clear param immediately after use       |
| Session not ready during auto-exec  | High   | Low         | Add proper guards and loading state     |
| Tool execution timeout              | Medium | Low         | Add timeout handling, error toasts      |
| Database migration failure          | High   | Low         | Test migration on dev DB first          |
| Type mismatch in playbook data      | Medium | Low         | Add runtime validation with type guards |

### User Experience Risks

| Risk                       | Impact | Probability | Mitigation                         |
| -------------------------- | ------ | ----------- | ---------------------------------- |
| Confusing auto-execution   | Medium | Medium      | Clear loading overlay with message |
| Lost context on error      | High   | Low         | Preserve playbook ID, allow retry  |
| Slow playbook list load    | Low    | Medium      | Add pagination, loading states     |
| Bookmark state out of sync | Medium | Low         | Optimistic UI updates              |

---

## Success Metrics

### Functionality

- ✅ All 9 manual test cases pass
- ✅ All 5 error scenarios handled gracefully
- ✅ Zero TypeScript errors
- ✅ Zero ESLint warnings
- ✅ Build completes successfully

### Performance

- ✅ Playbook list loads in < 500ms
- ✅ Auto-execution starts in < 1s
- ✅ No layout shift during load
- ✅ Smooth animations and transitions

### Code Quality

- ✅ Test coverage > 0% (initial baseline)
- ✅ No any types used
- ✅ All components documented
- ✅ No dead code remaining

---

## Rollout Plan

### Phase 1: Internal Testing (Day 7)

- Deploy to dev environment
- Manual testing by development team
- Fix any critical bugs

### Phase 2: Beta Release (Day 8-9)

- Deploy to beta environment
- Limited user testing
- Gather feedback

### Phase 3: Production Release (Day 10)

- Deploy to production
- Monitor error logs
- Quick response for issues

---

## Post-Implementation Tasks

### Week 2

- [ ] Monitor user adoption metrics
- [ ] Collect user feedback
- [ ] Fix any reported bugs
- [ ] Performance optimization if needed

### Future Enhancements

- [ ] Playbook preview modal
- [ ] Playbook sharing between users
- [ ] Playbook templates
- [ ] Playbook versioning
- [ ] Playbook analytics
- [ ] Export/import playbooks

---

## Resources & References

### Documentation

- [Idea Specification](../../idea.md)
- [Chat Feature Architecture](../architecture/chat-feature-architecture.md)
- [Agent Workflow Architecture](../architecture/agent-workflow-architecture.md)
- [UI Resource Implementation](../guides/ui-resource-implementation.md)

### Code References

- Assistant List Pattern: `src/features/assistant/List.tsx`
- Agent Session Management: `src/context/AgentSessionContext.tsx`
- Tool Message Creation: `src/lib/chat-utils.ts`
- Backend Playbook API: `src/lib/backend/playbooks.ts`

### External Resources

- [Tauri Command Documentation](https://tauri.app/v2/reference/commands/)
- [React Router Search Params](https://reactrouter.com/en/main/hooks/use-search-params)
- [SeaORM Migrations](https://www.sea-ql.org/SeaORM/docs/migration/writing-migration/)

---

## Approval & Sign-off

**Estimated Total Effort**: 9 days (72 hours)  
**Risk Level**: Low-Medium  
**Dependencies**: None (all infrastructure exists)  
**Blocker Status**: None

**Ready for Implementation**: ✅

---

## Change Log

| Date       | Author | Changes              |
| ---------- | ------ | -------------------- |
| 2026-01-21 | System | Initial plan created |
