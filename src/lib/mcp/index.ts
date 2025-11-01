/**
 * @file MCP Module Public API
 * @description Unified export for all MCP types and utilities
 *
 * This barrel export maintains backward compatibility while enabling
 * tree-shaking through granular imports.
 *
 * @example
 * // Old way (still works via mcp-types.ts)
 * import { MCPTool, MCPResponse } from '@/lib/mcp-types';
 *
 * // New way (preferred for tree-shaking)
 * import { MCPTool } from '@/lib/mcp/protocol';
 * import { MCPResponse } from '@/lib/mcp/protocol';
 *
 * // Or use barrel
 * import { MCPTool, MCPResponse } from '@/lib/mcp';
 */

// Schema types
export * from './schema';

// Protocol types
export * from './protocol';

// Configuration types
export * from './config';

// Web Worker types
export * from './web-worker';

// Utility functions
export * from './utils';
