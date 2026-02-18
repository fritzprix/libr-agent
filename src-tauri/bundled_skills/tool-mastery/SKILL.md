---
name: tool-mastery
description: Deep guide to mastering LibrAgent's builtin tools. Use when you need to understand tool capabilities, discover advanced usage patterns, or optimize your tool workflow for complex tasks.
---

# Tool Mastery Guide

You have access to powerful builtin tools. This guide helps you use them effectively.

## Tool Ecosystem Overview

LibrAgent provides ~88 tools across 12 servers. You don't need all of them at once. 
**Principle:** Use the minimum viable toolset for each task phase.

---

## Planning Tools (Your Brain)

**Purpose:** Track goals, todos, working memory

### Essential Workflow
```
createGoal("Build REST API")    → Set direction
addTodo("Design schema", 1)     → Step with priority (1=high)
addTodo("Implement endpoints")  → More steps
checkTodo(1)                    → Mark done by position
getCurrentState                 → See full picture
```

### Scratchpad (Working Memory)
```
addScratchpad("DB schema: users(id, name, email)")  → Save finding
listScratchpad                                       → See previews
readScratchpad("sp_abc123")                          → Read full content
updateScratchpad("sp_abc123", "Updated content")     → Modify
clearScratchpad("sp_abc123")                         → Remove
```

**Limit:** 10 items max. Save only what you'll reference again.

### Deep Thinking
```
pauseAndThink("Should I use SQL or NoSQL?")  → Structured reasoning
critiqueAndReflection                        → Self-assess progress
```

---

## Knowledge Tools (Your Long-Term Memory)

**Purpose:** Persist information across sessions

```
saveKnowledge(
  title: "Project DB Schema",
  content: "...",
  tags: ["database", "schema"]
)

searchKnowledge(query: "database")    → Full-text search
listKnowledge(page: 1, pageSize: 10)  → Browse all
readKnowledge(id: "k_abc123")         → Read specific
deleteKnowledge(id: "k_abc123")       → Remove
```

**Best Practice:**
- Tag everything for later discovery
- Save patterns, not raw data
- Search before re-investigating

---

## Workspace Tools (Your Hands)

### File Operations
```
listDirectory(path: ".")              → See what's here
readFile(path: "src/main.rs")         → Read file
readFile(path: "big.log", startLine: 100, endLine: 200)  → Read range
writeFile(path: "new.txt", content: "...")    → Create/overwrite
deleteFile(path: "temp.txt")          → Remove

searchFiles(pattern: "**/*.rs")       → Find files by glob
searchLineInFile(path: "app.rs", query: "fn main")  → Find text in file
```

### Editing (Precision Surgery)
```
editFile(
  path: "src/app.rs",
  oldContent: "let x = 1;",    → Exact match (include context!)
  newContent: "let x = 42;"    → Replacement
)

editFileMulti(                  → Multiple edits atomically
  path: "src/app.rs",
  edits: [
    { oldContent: "...", newContent: "..." },
    { oldContent: "...", newContent: "..." }
  ]
)

previewReplacement(...)         → Dry run before editing
```

**Critical Rule:** Always `readFile` before `editFile`. Never edit blind.

### Line-Based Editing
```
searchLineInFile(path: "app.rs", query: "TODO")  → Get line numbers
editLineInFile(
  path: "app.rs",
  edits: [{ lineNumber: 42, newContent: "// DONE" }]
)
```

### Shell Execution
```
# Isolated (stateless) - good for one-off commands
runShell(command: "ls -la")                    # Unix
runPowerShell(command: "Get-ChildItem")        # Windows

# Persistent (stateful) - preserves cd, env vars
runInPersistentShell(command: "cd /project")   # Unix
runInPersistentPowerShell(command: "cd C:\project")  # Windows

# Background processes
spawnProcess(command: "npm run dev")
listProcesses()
readProcessOutput(processId: "proc_123")
stopProcess(processId: "proc_123")
```

**Persistent vs Isolated:**
- Use persistent for: multi-step builds, stateful workflows
- Use isolated for: independent checks, simple queries

### Export
```
exportFile(path: "report.pdf")                → Single file download
exportZip(paths: ["src/", "README.md"])        → ZIP package
```

---

## Browser Tools (Your Eyes on the Web)

### Basic Navigation
```
createSession(sessionName: "research")
navigateToUrl(sessionId: "...", url: "https://docs.example.com")
getCurrentUrl(sessionId: "...")
getPageTitle(sessionId: "...")
```

### Content Extraction
```
extractWebContent(sessionId: "...")           → Full page as markdown
readWebContent(sessionId: "...", page: 2)     → Paginated reading
```

### Interaction
```
listInteractable(sessionId: "...")            → Find buttons/inputs
clickElement(sessionId: "...", selector: "#submit-btn")
inputText(sessionId: "...", selector: "#search", text: "query")
scrollPage(sessionId: "...", direction: "down")
```

### Cleanup
```
closeSession(sessionId: "...")
```

---

## Content Store (Document Library)

```
addContent(storeName: "research", content: "...", title: "Paper Summary")
listContent(storeName: "research")
readContent(storeName: "research", contentId: "...")
keywordSimilaritySearch(storeName: "research", query: "machine learning")
deleteContent(storeName: "research", contentId: "...")
```

**Use for:** Large documents, research collections, reference materials

---

## Session API (Multi-Agent Collaboration)

```
createChildSession(
  assistantId: "...",
  request: "Research market trends",
  workspacePath: "./research-team"
)

waitForSessionIdle(sessionId: "...")    → Wait for completion
getMessages(sessionId: "...")          → Read results
sendMessage(sessionId: "...", content: "Follow up on X")
terminateSession(sessionId: "...")     → Stop session
getChildSessions(sessionId: "...")     → List children
listAssistants                         → Find available specialists
```

---

## Tool Selection Matrix

| Task Type | Primary Tools | Supporting Tools |
|---|---|---|
| Research | browser, contentstore | knowledge, planning |
| Coding | workspace (read/edit/shell) | planning, knowledge |
| Writing | workspace (write) | contentstore, browser |
| Analysis | workspace (read), browser | planning, scratchpad |
| Automation | workspace (shell/process) | planning |
| Delegation | session_api | planning, knowledge |
| Setup/Config | bootstrap, mcp_manager | workspace |

## Golden Rules

1. **Read before write** — Always verify state before modifying
2. **Minimum viable toolset** — Don't activate tools you don't need
3. **Save before you forget** — External brain > internal memory
4. **One domain at a time** — Finish file ops before switching to browser
5. **Verify after action** — Check results, don't assume success
