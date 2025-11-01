/**
 * @file MCP Types (Legacy Compatibility Layer)
 *
 * @deprecated This file is deprecated and will be removed in a future version.
 * Please migrate to the new modular structure:
 *
 * - Schema types: import from '@/lib/mcp/schema'
 * - Protocol types: import from '@/lib/mcp/protocol'
 * - Config types: import from '@/lib/mcp/config'
 * - Web Worker types: import from '@/lib/mcp/web-worker'
 * - Utilities: import from '@/lib/mcp/utils'
 *
 * Or use the barrel export: import from '@/lib/mcp'
 *
 * @example Migration examples:
 * // Old (still works):
 * import { MCPTool, MCPResponse } from '@/lib/mcp-types';
 *
 * // New (preferred):
 * import { MCPTool, MCPResponse } from '@/lib/mcp/protocol';
 * // or
 * import { MCPTool, MCPResponse } from '@/lib/mcp';
 */

// Re-export everything from new modular structure for backward compatibility
export * from './mcp';

// This file can be removed once all imports are migrated
