# Message Search Test Guide

## Overview

This guide provides instructions for testing the new BM25-based message search functionality in SynapticFlow.

## Accessing the Search Feature

### Via Sidebar

1. Launch SynapticFlow: `pnpm tauri dev`
2. Look for the **"Message Search"** menu item in the sidebar under the **"History"** section
3. Click on it to navigate to `/search`

### Direct URL

Navigate to: `http://localhost:1420/search` (in development)

## UI Components

### Search Controls Card

- **Search Input**: Enter your search query
- **Session Selector**: Dropdown to select which session to search
- **Search Button**: Executes the search (also triggered by pressing Enter)

### Results Display

Each result shows:

- **BM25 Score**: Relevance score (higher = more relevant)
- **Session Name**: Which conversation the message is from
- **Message Snippet**: Preview of the message content with query context
- **Timestamp**: When the message was created
- **Message ID**: Unique identifier (for debugging)

### Pagination

- **Previous/Next Buttons**: Navigate through result pages
- **Page Indicator**: Shows current page and total pages
- Results per page: 25 (default)

## Test Scenarios

### 1. Basic Search Test

**Steps:**

1. Start a chat session and send several messages
2. Navigate to Message Search
3. Select the session from the dropdown
4. Enter a search query (e.g., "hello")
5. Click Search or press Enter

**Expected:**

- Results appear with BM25 scores
- Snippets contain the search term
- Messages are sorted by relevance

### 2. Empty Query Test

**Steps:**

1. Leave search input empty
2. Click Search

**Expected:**

- Error message: "Please enter a search query"
- No search is executed

### 3. No Session Selected Test

**Steps:**

1. Enter a search query
2. Don't select a session
3. Click Search

**Expected:**

- Error message: "Please select a session to search"
- No search is executed

### 4. No Results Test

**Steps:**

1. Search for a term that doesn't exist in any messages (e.g., "xyzabc123")

**Expected:**

- Empty state message: "No results found"
- Suggestion to try a different query

### 5. Pagination Test

**Steps:**

1. Create a session with 50+ messages containing the same keyword
2. Search for that keyword
3. Navigate through pages using Previous/Next buttons

**Expected:**

- Page indicator updates correctly
- Previous/Next buttons enable/disable appropriately
- Each page shows different results

### 6. Multilingual Search Test (Korean)

**Steps:**

1. Send messages in Korean: "안녕하세요", "감사합니다", "반가워요"
2. Search for: "안녕"

**Expected:**

- Results show Korean messages
- Snippets display correctly
- Note: BM25 tokenization is currently optimized for English

### 7. Loading State Test

**Steps:**

1. Search for a common term in a large session
2. Observe the loading spinner

**Expected:**

- Loading spinner appears immediately
- "Searching messages..." text displays
- Search button is disabled during loading

### 8. Retry on Error Test

**Steps:**

1. Simulate an error (search with invalid session ID via browser console)
2. Click the Retry button

**Expected:**

- Error banner shows with descriptive message
- Retry button executes the same search
- Results appear if error was transient

## Performance Benchmarks

### Index Building

- **Small sessions** (<100 messages): Instant (~10-50ms)
- **Medium sessions** (100-1000 messages): Fast (~50-200ms)
- **Large sessions** (1000-10000 messages): Quick (~200-1000ms)

### Search Execution

- **Simple queries**: ~5-20ms
- **Complex queries**: ~20-100ms
- **With pagination**: ~5-20ms per page

## Environment Variables

### MESSAGE_INDEX_MAX_DOCS

Controls the maximum number of messages to index per session.

**Default**: 10,000 messages

**How to test different limits:**

```bash
# In src-tauri/.env or shell
export MESSAGE_INDEX_MAX_DOCS=1000
pnpm tauri dev
```

**Expected behavior:**

- Only the most recent N messages are indexed
- Older messages won't appear in search results
- Index build time is faster for large sessions

## Backend Testing

### Check Index Files

Indices are stored in:

```
~/.local/share/com.fritzprix.synapticflow/message_indices/
```

Each session has a `.idx` file containing the BM25 index.

### Database Inspection

Check `message_index_meta` table:

```sql
sqlite3 ~/.local/share/com.fritzprix.synapticflow/synapticflow_v2.db

SELECT * FROM message_index_meta;
```

**Columns:**

- `session_id`: Session identifier
- `index_path`: File path to the index
- `last_indexed_at`: Timestamp of last rebuild (Unix milliseconds)
- `doc_count`: Number of documents in the index
- `index_version`: Schema version
- `last_rebuild_duration_ms`: How long the last rebuild took

### Background Worker

The background worker runs every 5 minutes to reindex dirty sessions.

**Check logs:**

```
~/.local/share/com.fritzprix.synapticflow/logs/synaptic-flow.log
```

Look for:

- "🔄 Message search indexing worker started"
- "🔨 Rebuilding index for session: {session_id}"
- "✅ Index rebuilt for session: {session_id}"

## Common Issues

### Issue: No results found for known terms

**Possible causes:**

- Index hasn't been built yet (first search triggers build)
- Session has no messages
- Query doesn't match message content exactly

**Solution:**

- Try again after a few seconds
- Check session actually contains messages
- Try broader search terms

### Issue: Search is slow

**Possible causes:**

- Very large session (>10k messages)
- Index is being rebuilt

**Solution:**

- Wait for index build to complete
- Reduce `MESSAGE_INDEX_MAX_DOCS` if needed
- Check `last_rebuild_duration_ms` in database

### Issue: Korean/multilingual results not good

**Known limitation:**

- BM25 tokenization is currently English-optimized
- Future enhancement: Add morphological analysis for Korean

**Workaround:**

- Use space-separated keywords
- Try shorter query terms

## Success Criteria

✅ All UI components render correctly
✅ Search executes successfully
✅ Results show relevant messages with scores
✅ Pagination works correctly
✅ Error handling displays appropriate messages
✅ Loading states work
✅ Performance is acceptable (<1s for typical searches)
✅ Background indexing runs without errors

## Reporting Issues

If you find bugs, please include:

1. Steps to reproduce
2. Expected vs actual behavior
3. Session size (number of messages)
4. Search query used
5. Browser console errors (F12)
6. Backend logs (if available)

## Next Steps

After testing, consider:

1. Adding highlighting to snippets
2. Implementing cross-session search
3. Adding filters (date range, role, etc.)
4. Improving multilingual tokenization
5. Adding search history/suggestions
