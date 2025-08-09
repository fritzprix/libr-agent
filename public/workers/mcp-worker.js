/**
 * 🌐 Web Worker MCP Server - Public Entry Point
 *
 * This worker runs MCP servers in a web worker environment,
 * providing MCP-compatible functionality without Node.js/Python dependencies.
 */

// Import the main worker implementation
// Note: This will be bundled by Vite during build
import('/src/lib/web-mcp/mcp-worker.ts').then(() => {
  console.log('[MCP Worker] Worker module loaded successfully');
}).catch((error) => {
  console.error('[MCP Worker] Failed to load worker module:', error);

  // Send error message to main thread
  self.postMessage({
    id: 'init-error',
    error: `Failed to load worker module: ${error.message}`
  });
});
