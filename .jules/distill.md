## 2026-04-08 - Pagination Strategy

**Learning:** Returning an unpaginated raw JSON array dumps excessive tokens into the LLM context, but completely dropping records confuses the agent if they are needed later.
**Action:** When implementing pagination, always return `offset` and `total_matches` alongside `paginated_results`. Always convert the paginated JSON results into a dense Markdown table (like `| Type | Path | Size |`) injected into the text output rather than just returning raw JSON. Add explicit truncation notes (e.g. `*(Showing 1 to 50 of 120 total matches. Call search with offset: 50 to see more)*`) to explicitly guide the LLM on how to fetch more.
