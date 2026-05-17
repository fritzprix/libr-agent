## 2024-05-17 - Optimize Legacy Session API Responses
**Learning:** Legacy swarm/session API endpoints (getChildAgents, listAgentTypes) return unpaginated lists. If an org scales to 100+ agents, this instantly bloats the context window. Converting these directly into heavily sanitized markdown tables with explicit pagination protects the token budget while keeping data actionable.
**Action:** Always wrap dynamically generated external lists and configuration dumps in a `skip(offset).take(limit)` iterator and sanitize pipes/newlines before emitting a Markdown table.
