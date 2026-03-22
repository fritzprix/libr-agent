# Dynamic Tool Classification Logic

When surveying tools using `tool__list`, use this logic to categorize them based on their `description` and `name`.

## Category: Search & Research
- **Keywords**: "search", "lookup", "research", "news", "trend", "discover", "academic", "papers".
- **Matching Pattern**: If description contains "web search", "HackerNews", "arxiv", or "retrieve information".
- **Primary Utility**: Information gathering and verification.

## Category: Financial & Economic Data
- **Keywords**: "finance", "stock", "economic", "GDP", "market", "prices", "FRED".
- **Matching Pattern**: If description mentions financial indicators, stock markets, or federal reserve data.
- **Primary Utility**: Strategic analysis and market forecasting.

## Category: Technical & Engineering
- **Keywords**: "code", "coding", "library", "docs", "documentation", "GitHub", "software".
- **Matching Pattern**: If description involves autonomous coding, library reference lookup, or repository management.
- **Primary Utility**: Implementation and debugging.

## Category: AI & Multimedia
- **Keywords**: "AI", "generation", "image", "video", "multimodal", "analysis", "synthesis".
- **Matching Pattern**: If description includes image generation, media analysis, or LLM-based content creation.
- **Primary Utility**: Creative output and complex content analysis.

## Pruning Logic (Cleanup)
- **Dead References**: Any MCP Server ID present in an agent but **NOT** in the current `tool__list` must be marked for **REMOVAL**.
- **Duplicated Capability**: If two tools provide near-identical capability (e.g., two web search tools), check if the agent description justifies having both.
