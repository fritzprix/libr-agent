---
name: team-protocols
description: Patterns for multi-agent collaboration and delegation. Use when coordinating work across specialist agents, managing parent-child session relationships, or designing workflows that span multiple AI agents.
---

# Team Protocols

Guide for effective multi-agent collaboration within LibrAgent.

## Team Architecture

LibrAgent uses a hierarchical session model:
```
Master Mind (Strategy & Coordination)
├── Libr Assistant (General Operations)
├── Coding Expert (Implementation)
├── App Wizard (Environment Setup)
└── Custom Specialists (User-Created)
```

Each agent runs in an isolated session with its own workspace, planning state, and knowledge base.

## Communication Channels

Agents communicate ONLY through the Session API:
```
sendMessage(sessionId, content)     → Send task/question
getMessages(sessionId)              → Read responses
waitForSessionIdle(sessionId)       → Wait for completion
```

**No shared memory.** Each agent's knowledge/planning is private.
**No direct tool access.** You can't use another agent's tools.

## Delegation Patterns

### Pattern 1: Fire and Forget

Best for independent, self-contained tasks.

```
1. createChildSession(assistantId, request: "Complete task X")
2. Continue your own work
3. Later: waitForSessionIdle → getMessages → integrate result
```

**When to use:** Research tasks, independent analysis, parallel work

### Pattern 2: Supervised Execution

Best for critical tasks requiring quality control.

```
1. createChildSession(assistantId, request: "Do X with constraints Y")
2. waitForSessionIdle(sessionId)
3. getMessages(sessionId) → Review output
4. If insufficient: sendMessage(sessionId, "Revise: ...")
5. Repeat until satisfactory
```

**When to use:** Code changes, important decisions, user-facing outputs

### Pattern 3: Pipeline

Best for sequential processing stages.

```
1. Agent A: Research → produces summary
2. Read Agent A results
3. Agent B: Implement based on A's findings
4. Read Agent B results  
5. Agent C: Test and validate B's implementation
```

**When to use:** Multi-phase projects, staged workflows

### Pattern 4: Parallel Fan-Out

Best for gathering diverse inputs quickly.

```
1. Spawn agents A, B, C simultaneously
2. Each investigates different angle
3. waitForSessionIdle on all three
4. Collect and synthesize results
5. Make informed decision
```

**When to use:** Competitive analysis, multi-perspective evaluation

## Crafting Good Delegation Requests

### ❌ Bad Request
```
"Look into the database stuff"
```

### ✅ Good Request
```
"Analyze the SQLite schema in workspace/data.db:
1. List all tables and their columns
2. Identify foreign key relationships  
3. Check for missing indexes on frequently queried columns
4. Report findings as a structured summary

Save key findings to knowledge for team reference."
```

### Request Template
```
OBJECTIVE: [What to accomplish]
CONTEXT: [Background information needed]
CONSTRAINTS: [Boundaries, time limits, scope limits]
DELIVERABLE: [Exact format of expected output]
SAVE TO: [Where to persist results - knowledge/workspace/report]
```

## Result Integration

When receiving results from child sessions:

```
1. getMessages(childSessionId, budget: 5000)
2. Evaluate quality and completeness
3. Extract actionable information
4. Save critical findings to YOUR scratchpad/knowledge
5. Incorporate into your own workflow
```

**Don't blindly trust.** Verify claims when stakes are high.

## Workspace Isolation

Each session has its own workspace directory:
```
Parent workspace: /workspaces/session-abc/
Child workspace:  /workspaces/session-def/  (separate!)
```

### Sharing Files Between Agents
- Write file to a shared path before delegating
- Tell child agent where to find it in the request
- Or create child with custom workspacePath pointing to shared dir

### Creating Dedicated Workspaces
```
createChildSession(
  assistantId: "coding-expert-id",
  request: "Build the API server",
  workspacePath: "./api-server"    → Child gets dedicated workspace
)
```

## Error Handling

### Child Session Failures
```
1. waitForSessionIdle(sessionId) → check status
2. If failed: getMessages → understand what went wrong
3. Options:
   a. sendMessage with corrective instructions
   b. terminateSession and retry with different approach
   c. Handle the task yourself
```

### Communication Timeouts
```
1. waitForSessionIdle with reasonable expectations
2. If stuck: getSession → check if still running
3. If truly stuck: terminateSession → retry or escalate
```

## Team Etiquette

1. **Be specific in requests** — Vague tasks produce vague results
2. **Include context** — The child doesn't know what you know
3. **Set clear success criteria** — "Done" should be unambiguous
4. **Don't micro-manage** — Trust specialists within their domain
5. **Save shared discoveries** — Write to workspace files others can access
6. **Clean up** — Terminate sessions when work is complete
7. **Report honestly** — State what you found, including failures

## Quick Reference

```
DELEGATE:
  listAssistants → find specialist
  createChildSession(assistantId, request, workspacePath?)
  
MONITOR:
  getChildSessions → list all children
  getSession(id) → check status
  waitForSessionIdle(id) → block until done
  
COMMUNICATE:
  getMessages(id) → read results
  sendMessage(id, content) → follow up
  
CLEANUP:
  terminateSession(id) → stop session
```
