/**
 * @file MCP Tool Types
 * @description Tool definition and annotation types
 */

import type { JSONSchemaObject } from '../schema';

/**
 * Defines annotations that provide additional metadata about an MCP tool.
 */
export interface MCPToolAnnotations {
  /** Specifies the intended audience for the tool's output. */
  audience?: ('user' | 'assistant')[];
  /** A priority level for the tool, can be used for sorting or selection. */
  priority?: number;
  /** The timestamp of when the tool was last modified. */
  lastModified?: string;
  /** Allows for other custom annotations. */
  [key: string]: unknown;
}

/**
 * Represents a tool that can be invoked through the MCP.
 */
export interface MCPTool {
  /** The unique name of the tool. */
  name: string;
  /** A human-readable title for the tool. */
  title?: string;
  /** A detailed description of what the tool does. */
  description: string;
  /** The JSON Schema for the tool's input parameters. */
  inputSchema: JSONSchemaObject;
  /** The JSON Schema for the tool's output. */
  outputSchema?: JSONSchemaObject;
  /** Additional metadata about the tool. */
  annotations?: MCPToolAnnotations;
  /** Specifies where the tool is executed. */
  backend?: 'tauri' | 'webworker';
}
