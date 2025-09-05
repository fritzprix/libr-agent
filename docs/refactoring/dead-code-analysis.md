# Dead Code Analysis Notes

## Known False Positives

### Vite Worker Imports
The following "unresolved imports" are **expected and safe**:
- `../lib/web-mcp/mcp-worker.ts?worker`
- `../../lib/web-mcp/mcp-worker.ts?worker`

These use Vite's `?worker` import syntax which is the standard way to load Web Workers in Vite. The `unimported` tool cannot resolve these patterns, but they are valid and should not be removed.

## Protected Files

The following files are intentionally protected from dead code analysis:
- **Web Workers**: `**/mcp-worker.ts`
- **MCP Modules**: `**/web-mcp/modules/**`
- **UI Components**: `alert-dialog.tsx`, `collapsible.tsx`, `sonner.tsx`
- **Test Setup**: `src/test/setup.ts`

## Usage

```bash
# Run dead code analysis
pnpm dead-code

# Full refactor validation
pnpm refactor:validate
```

Remember: Vite Worker imports showing as "unresolved" are normal and expected.
