import { invoke } from '@tauri-apps/api/core';

export interface MCPServerConfig {
  name: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  transport: 'stdio' | 'http' | 'websocket';
  url?: string;
  port?: number;
}

// JSON Schema primitive types
export type JSONSchemaType =
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'object'
  | 'array'
  | 'null';

// Base JSON Schema definition
export interface JSONSchemaBase {
  type?: JSONSchemaType | JSONSchemaType[];
  title?: string;
  description?: string;
  default?: unknown;
  examples?: unknown[];
  enum?: unknown[];
  const?: unknown;
}

// String-specific schema properties
export interface JSONSchemaString extends JSONSchemaBase {
  type: 'string';
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?:
    | 'date-time'
    | 'date'
    | 'time'
    | 'email'
    | 'hostname'
    | 'ipv4'
    | 'ipv6'
    | 'uri'
    | 'uri-reference'
    | 'uuid'
    | string;
}

// Number-specific schema properties
export interface JSONSchemaNumber extends JSONSchemaBase {
  type: 'number' | 'integer';
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}

// Boolean schema
export interface JSONSchemaBoolean extends JSONSchemaBase {
  type: 'boolean';
}

// Null schema
export interface JSONSchemaNull extends JSONSchemaBase {
  type: 'null';
}

// Array schema
export interface JSONSchemaArray extends JSONSchemaBase {
  type: 'array';
  items?: JSONSchema | JSONSchema[];
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
  additionalItems?: boolean | JSONSchema;
}

// Object schema
export interface JSONSchemaObject extends JSONSchemaBase {
  type: 'object';
  properties?: Record<string, JSONSchema>;
  required?: string[];
  additionalProperties?: boolean | JSONSchema;
  patternProperties?: Record<string, JSONSchema>;
  minProperties?: number;
  maxProperties?: number;
  dependencies?: Record<string, JSONSchema | string[]>;
}

// Union type for all possible JSON Schema definitions
export type JSONSchema =
  | JSONSchemaString
  | JSONSchemaNumber
  | JSONSchemaBoolean
  | JSONSchemaNull
  | JSONSchemaArray
  | JSONSchemaObject
  | (JSONSchemaBase & { type?: JSONSchemaType | JSONSchemaType[] }); // For mixed or unspecified types

// MCP Tool annotations for metadata about tool behavior
export interface MCPToolAnnotations {
  audience?: ('user' | 'assistant')[];
  priority?: number;
  lastModified?: string;
  [key: string]: unknown;
}

// Helper functions for creating JSON Schema objects
export function createStringSchema(options?: {
  description?: string;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
}): JSONSchemaString {
  return {
    type: 'string',
    ...options,
  };
}

export function createNumberSchema(options?: {
  description?: string;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}): JSONSchemaNumber {
  return {
    type: 'number',
    ...options,
  };
}

export function createIntegerSchema(options?: {
  description?: string;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}): JSONSchemaNumber {
  return {
    type: 'integer',
    ...options,
  };
}

export function createBooleanSchema(options?: {
  description?: string;
}): JSONSchemaBoolean {
  return {
    type: 'boolean',
    ...options,
  };
}

export function createArraySchema(options?: {
  description?: string;
  items?: JSONSchema;
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
}): JSONSchemaArray {
  return {
    type: 'array',
    ...options,
  };
}

export function createObjectSchema(options?: {
  description?: string;
  properties?: Record<string, JSONSchema>;
  required?: string[];
  additionalProperties?: boolean;
}): JSONSchemaObject {
  return {
    type: 'object',
    ...options,
  };
}

// Complete MCP Tool definition following the specification
export interface MCPTool {
  name: string;
  title?: string;
  description: string;
  inputSchema: JSONSchemaObject;
  outputSchema?: JSONSchemaObject;
  annotations?: MCPToolAnnotations;
}

export interface ToolCallResult {
  success: boolean;
  result?: unknown;
  error?: string;
}

export class TauriMCPClient {
  async startServer(config: MCPServerConfig): Promise<string> {
    return await invoke('start_mcp_server', { config });
  }

  async stopServer(serverName: string): Promise<void> {
    return await invoke('stop_mcp_server', { serverName });
  }

  async callTool(
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<ToolCallResult> {
    return await invoke('call_mcp_tool', {
      serverName,
      toolName,
      arguments: arguments_,
    });
  }

  async listTools(serverName: string): Promise<MCPTool[]> {
    return await invoke('list_mcp_tools', { serverName });
  }

  async listToolsFromConfig(config: {
    mcpServers?: Record<
      string,
      { command: string; args?: string[]; env?: Record<string, string> }
    >;
  }): Promise<MCPTool[]> {
    return await invoke('list_tools_from_config', { config });
  }

  async getConnectedServers(): Promise<string[]> {
    return await invoke('get_connected_servers');
  }

  async checkServerStatus(serverName: string): Promise<boolean> {
    return await invoke('check_server_status', { serverName });
  }

  async checkAllServersStatus(): Promise<Record<string, boolean>> {
    return await invoke('check_all_servers_status');
  }
}

export const tauriMCPClient = new TauriMCPClient();
