/**
 * @file MCP (Model Context Protocol) Type Definitions
 *
 * @description
 * This file centralizes all type definitions for the Model Context Protocol (MCP).
 * It adheres to the MCP specification and is intended to be the single source of truth
 * for all MCP-related types used throughout the application.
 *
 * @see {@link https://modelcontextprotocol.io/|MCP Specification}
 */
import { UIResource } from '@mcp-ui/server';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';

// ========================================
// JSON Schema Types (Adhering to MCP Specification)
// ========================================

/**
 * Represents the possible data types in a JSON Schema.
 */
export type JSONSchemaType =
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'object'
  | 'array'
  | 'null';

/**
 * The base interface for all JSON Schema definitions.
 */
export interface JSONSchemaBase {
  /** The data type of the schema. */
  type?: JSONSchemaType | JSONSchemaType[];
  /** A title for the schema. */
  title?: string;
  /** A description of the schema. */
  description?: string;
  /** The default value for the schema. */
  default?: unknown;
  /** An array of example values. */
  examples?: unknown[];
  /** An array of allowed values. */
  enum?: unknown[];
  /** A constant value that the schema must have. */
  const?: unknown;
}

/**
 * Represents a JSON Schema for a string type.
 */
export interface JSONSchemaString extends JSONSchemaBase {
  type: 'string';
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
}

/**
 * Represents a JSON Schema for a number or integer type.
 */
export interface JSONSchemaNumber extends JSONSchemaBase {
  type: 'number' | 'integer';
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}

/**
 * Represents a JSON Schema for a boolean type.
 */
export interface JSONSchemaBoolean extends JSONSchemaBase {
  type: 'boolean';
}

/**
 * Represents a JSON Schema for a null type.
 */
export interface JSONSchemaNull extends JSONSchemaBase {
  type: 'null';
}

/**
 * Represents a JSON Schema for an array type.
 */
export interface JSONSchemaArray extends JSONSchemaBase {
  type: 'array';
  items?: JSONSchema | JSONSchema[];
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
  additionalItems?: boolean | JSONSchema;
}

/**
 * Represents a JSON Schema for an object type.
 */
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

/**
 * A union type representing any valid JSON Schema.
 */
export type JSONSchema =
  | JSONSchemaString
  | JSONSchemaNumber
  | JSONSchemaBoolean
  | JSONSchemaNull
  | JSONSchemaArray
  | JSONSchemaObject
  | (JSONSchemaBase & { type?: JSONSchemaType | JSONSchemaType[] });

// ========================================
// MCP Content Types (Adhering to Specification)
// ========================================

/**
 * Represents a text content part in an MCP message.
 */
export interface MCPTextContent {
  type: 'text';
  text: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents an image content part in an MCP message.
 */
export interface MCPImageContent {
  type: 'image';
  /** The image data encoded in base64. */
  data: string;
  mimeType: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents an audio content part in an MCP message.
 */
export interface MCPAudioContent {
  type: 'audio';
  /** The audio data encoded in base64. */
  data: string;
  mimeType: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents a link to an external resource in an MCP message.
 */
export interface MCPResourceLinkContent {
  type: 'resource_link';
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents a rich UI resource, extending the base `UIResource` type
 * with optional service information. This consolidates previous resource types.
 */
type MCPResourceContent = UIResource & {
  serviceInfo?: ServiceInfo;
};

/**
 * A union type representing any valid MCP content part.
 */
export type MCPContent =
  | MCPTextContent
  | MCPImageContent
  | MCPAudioContent
  | MCPResourceLinkContent
  | MCPResourceContent;

// ========================================
// Service Context Types (for tool resolution)
// ========================================

/**
 * Provides information about the service that generated a content part,
 * which is crucial for resolving tool calls correctly.
 */
export interface ServiceInfo {
  /** The name of the server that provided the tool. */
  serverName: string;
  /** The name of the tool that was used. */
  toolName: string;
  /** The type of backend where the tool was executed. */
  backendType: 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';
}

/**
 * A type guard to check if an MCPContent object has service information.
 * @param content The content object to check.
 * @returns True if the content has service information, false otherwise.
 */
export function hasServiceInfo(
  content: MCPContent,
): content is MCPContent & { serviceInfo: ServiceInfo } {
  return (
    content &&
    typeof content === 'object' &&
    'serviceInfo' in content &&
    content.serviceInfo !== undefined
  );
}

/**
 * Extracts the first `ServiceInfo` object found in an array of MCP content parts.
 * @param content An array of MCPContent objects.
 * @returns The first `ServiceInfo` object found, or null if none exist.
 */
export function extractServiceInfoFromContent(
  content: MCPContent[],
): ServiceInfo | null {
  for (const item of content) {
    if (hasServiceInfo(item)) {
      return item.serviceInfo;
    }
  }
  return null;
}

// ========================================
// MCP Protocol Types (Adhering to JSON-RPC 2.0)
// ========================================

/**
 * Defines options for text generation (sampling).
 */
export interface SamplingOptions {
  model?: string;
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  topK?: number;
  stopSequences?: string[];
  presencePenalty?: number;
  frequencyPenalty?: number;
}

/**
 * Represents a request for text generation.
 */
export interface SamplingRequest {
  prompt: string;
  options?: SamplingOptions;
}

/**
 * Represents the result part of a standard MCP response.
 * @template T The type of the structured content.
 */
export interface MCPResult<T = unknown> {
  content?: MCPContent[];
  structuredContent?: T;
  /** A flag indicating if the result is from a tool execution that resulted in an error. */
  isError?: boolean;
}

/**
 * Extends the standard MCP result with information specific to a sampling operation.
 */
export interface SamplingResult extends MCPResult {
  sampling?: {
    finishReason?: 'stop' | 'length' | 'tool_use' | 'error';
    usage?: {
      promptTokens: number;
      completionTokens: number;
      totalTokens: number;
    };
    model?: string;
  };
}

/**
 * Represents a JSON-RPC error object.
 */
export interface MCPError {
  code: number;
  message: string;
  data?: unknown;
}

/**
 * Represents a response to a sampling request.
 */
export interface SamplingResponse extends MCPResponse<unknown> {
  result?: SamplingResult;
}

/**
 * The standard MCP response structure, compliant with JSON-RPC 2.0.
 * All MCP responses must follow this format.
 * @template T The type of the structured content in the result.
 */
export interface MCPResponse<T> {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult<T> | SamplingResult;
  error?: MCPError;
}

/**
 * An extended MCP response that includes service context information.
 * This preserves the service context to support accurate tool re-invocation from the UI.
 */
export interface ExtendedMCPResponse extends MCPResponse<unknown> {
  serviceInfo?: {
    serverName: string;
    toolName: string;
    backendType: 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';
  };
}

/**
 * A type guard to check if a response is an `ExtendedMCPResponse`.
 * @param response The response object to check.
 * @returns True if the response is an `ExtendedMCPResponse`, false otherwise.
 */
export function isExtendedResponse(
  response: MCPResponse<unknown>,
): response is ExtendedMCPResponse {
  return response && typeof response === 'object' && 'serviceInfo' in response;
}

// ========================================
// MCP Tool Types
// ========================================

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

// ========================================
// Server Configuration
// ========================================

/**
 * Defines the configuration for launching and connecting to an MCP server.
 * This is the legacy format, kept for backward compatibility.
 * New code should use MCPServerConfigV2 from @/models/chat instead.
 */
export interface MCPServerConfig {
  /** The unique name of the server. */
  name: string;
  /** The command to execute to start the server. */
  command?: string;
  /** An array of arguments to pass to the command. */
  args?: string[];
  /** Environment variables to set for the server process. */
  env?: Record<string, string>;
  /** The transport protocol used to communicate with the server. */
  transport: 'stdio' | 'http' | 'websocket';
  /** The URL of the server, for http or websocket transports. */
  url?: string;
  /** The port number for http or websocket transports. */
  port?: number;
}

// ========================================
// Unified MCP Types (Tauri + Web Worker)
// ========================================

/**
 * Defines the possible types of MCP servers.
 */
export type MCPServerType = 'tauri' | 'webworker';

/**
 * A unified configuration for an MCP server, whether it's a Tauri-based
 * backend process or a Web Worker-based server.
 */
export interface UnifiedMCPServerConfig {
  /** The unique name of the server. */
  name: string;
  /** The type of the server. */
  type: MCPServerType;
  // Properties for Tauri-based servers
  /** The command to execute to start the server. */
  command?: string;
  /** An array of arguments to pass to the command. */
  args?: string[];
  /** Environment variables to set for the server process. */
  env?: Record<string, string>;
  /** The transport protocol used to communicate with the server. */
  transport?: 'stdio' | 'http' | 'websocket';
  /** The URL of the server. */
  url?: string;
  /** The port number of the server. */
  port?: number;
  // Properties for Web Worker-based servers
  /** The path to the worker module to load. */
  modulePath?: string;
  /** The path to the main worker script. */
  workerPath?: string;
}

/**
 * Defines the context for executing a tool in a unified MCP environment.
 */
export interface MCPToolExecutionContext {
  /** The type of server where the tool will be executed. */
  serverType: MCPServerType;
  /** The name of the server to use. */
  serverName: string;
  /** The name of the tool to execute. */
  toolName: string;
  /** The arguments to pass to the tool. */
  arguments: unknown;
  /** An optional timeout for the tool execution in milliseconds. */
  timeout?: number;
}

/**
 * Creates a JSON schema for a string.
 * @param options Optional constraints for the string schema.
 * @returns A `JSONSchemaString` object.
 */
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

/**
 * Creates a JSON schema for a number.
 * @param options Optional constraints for the number schema.
 * @returns A `JSONSchemaNumber` object.
 */
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

/**
 * Creates a JSON schema for an integer.
 * @param options Optional constraints for the integer schema.
 * @returns A `JSONSchemaNumber` object with type 'integer'.
 */
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

/**
 * Creates a JSON schema for a boolean.
 * @param options Optional description for the boolean schema.
 * @returns A `JSONSchemaBoolean` object.
 */
export function createBooleanSchema(options?: {
  description?: string;
}): JSONSchemaBoolean {
  return {
    type: 'boolean',
    ...options,
  };
}

/**
 * Creates a JSON schema for an array.
 * @param options Optional constraints for the array schema.
 * @returns A `JSONSchemaArray` object.
 */
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

/**
 * Creates a JSON schema for an object.
 * @param options Optional constraints for the object schema.
 * @returns A `JSONSchemaObject` object.
 */
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

/**
 * A type guard to check if an MCP response is a success response.
 * @param response The MCP response to check.
 * @returns True if the response has a `result` property and no `error` property.
 */
export function isMCPSuccess(
  response: MCPResponse<unknown>,
): response is MCPResponse<unknown> & { result: MCPResult } {
  return response.error === undefined && response.result !== undefined;
}

/**
 * A type guard to check if an MCP response is an error response.
 * @param response The MCP response to check.
 * @returns True if the response has an `error` property.
 */
export function isMCPError(
  response: MCPResponse<unknown>,
): response is MCPResponse<unknown> & { error: MCPError } {
  return response.error !== undefined;
}

/**
 * Checks if an `MCPResult` object contains any valid content.
 * @param result The result object to check.
 * @returns True if the result has either `content` or `structuredContent`.
 */
export function isValidMCPResult(result: MCPResult): boolean {
  return !!(result.content?.length || result.structuredContent);
}

/**
 * Safely extracts the `structuredContent` from an MCP response.
 * @template T The expected type of the structured content.
 * @param response The MCP response.
 * @returns The `structuredContent` if it exists, otherwise null.
 */
export function extractStructuredContent<T>(
  response: MCPResponse<T>,
): T | null {
  if (!response.result || response.error) {
    return null;
  }

  // This is not a standard MCPResult, but a SamplingResult, so it can't have structuredContent.
  if ('sampling' in response.result) {
    return null;
  }

  return (response.result as MCPResult<T>).structuredContent || null;
}

/**
 * A type guard to check if an MCP response is successful and contains structured content.
 * @template T The expected type of the structured content.
 * @param response The MCP response to check.
 * @returns True if the response is successful and has `structuredContent`.
 */
export function hasStructuredContent<T>(
  response: MCPResponse<T>,
): response is MCPResponse<T> & {
  result: MCPResult<T> & { structuredContent: T };
} {
  const structured = extractStructuredContent(response);
  return structured !== null && structured !== undefined;
}

// ========================================
// Web Worker MCP Types
// ========================================

/**
 * Defines the interface for an MCP server running in a Web Worker.
 */
export interface WebMCPServer {
  /** The name of the server. */
  name: string;
  /** An optional description of the server. */
  description?: string;
  /** The version of the server. */
  version?: string;
  /** An array of tools provided by the server. */
  tools: MCPTool[];
  /**
   * A function to call a tool on the server.
   * @param name The name of the tool to call.
   * @param args The arguments for the tool.
   * @returns A promise that resolves to an MCP response.
   */
  callTool: (name: string, args: unknown) => Promise<MCPResponse<unknown>>;
  /**
   * An optional function to perform text sampling.
   * @param prompt The prompt to use for sampling.
   * @param options Optional sampling parameters.
   * @returns A promise that resolves to a sampling response.
   */
  sampleText?: (
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
  /** An optional function to get the service context. */
  getServiceContext?: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;
  /**
   * An optional function to switch the context for the server.
   * This allows servers to maintain state based on external context like session IDs or assistant IDs.
   * @param context The context object containing session/assistant identifiers and other state.
   * @returns A promise that resolves when the context is switched.
   */
  switchContext?: (context: ServiceContextOptions) => Promise<void>;
}

/**
 * Defines the structure of messages sent to and from a Web Worker MCP server.
 */
export interface WebMCPMessage {
  /** A unique identifier for the message. */
  id: string;
  /** The type of the message, indicating the requested action. */
  type:
    | 'listTools'
    | 'callTool'
    | 'ping'
    | 'loadServer'
    | 'sampleText'
    | 'getServiceContext'
    | 'setContext'
    | 'switchContext';
  /** The name of the server, for loading specific servers. */
  serverName?: string;
  /** The name of the tool to call. */
  toolName?: string;
  /** The arguments for the tool call. */
  args?: unknown;
}

/**
 * Defines the configuration for the Web Worker MCP proxy.
 */
export interface WebMCPProxyConfig {
  /** The path to the worker script. */
  workerPath?: string;
  /** An existing worker instance to use. */
  workerInstance?: Worker;
  /** The timeout for requests in milliseconds. */
  timeout?: number;
  /** Options for retrying failed requests. */
  retryOptions?: {
    maxRetries?: number;
    baseDelay?: number;
    maxDelay?: number;
    timeout?: number;
  };
}

/**
 * Represents the state of a Web Worker MCP server.
 */
export interface WebMCPServerState {
  /** Indicates if the server's tools are loaded. */
  loaded: boolean;
  /** The list of tools provided by the server. */
  tools: MCPTool[];
  /** The last error message received from the server. */
  lastError?: string;
  /** The timestamp of the last activity from the server. */
  lastActivity?: number;
}

// ========================================
// V2 MCP Configuration Types (MCP 2025-06-18 Spec)
// ========================================

/**
 * Discriminated union for transport-specific configurations
 */
export type TransportConfig =
  | {
      type: 'stdio';
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  | {
      type: 'http';
      url: string;
      protocolVersion?: string; // Default: "2025-06-18"
      sessionId?: string;
      headers?: Record<string, string>;
      enableSSE?: boolean; // For backward compatibility with older servers
      security?: {
        enableDnsRebindingProtection?: boolean;
        allowedOrigins?: string[];
        allowedHosts?: string[];
      };
    };

/**
 * OAuth 2.1 authentication configuration (RFC 8414, RFC 7636)
 */
export interface OAuthConfig {
  type: 'oauth2.1';
  discoveryUrl?: string; // RFC 8414 Authorization Server Metadata
  authorizationEndpoint?: string; // Fallback if discovery not available
  tokenEndpoint?: string;
  registrationEndpoint?: string; // RFC 7591 Dynamic Client Registration
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
  usePKCE?: boolean; // Default: true
  resourceParameter?: string; // RFC 9728 Protected Resource Metadata
}

/**
 * Server metadata (optional descriptive information)
 */
export interface ServerMetadata {
  description?: string;
  vendor?: string;
  version?: string;
}

/**
 * V2 MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
 */
export interface MCPServerConfigV2 {
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
}

/**
 * Legacy MCP Server Configuration (stdio-only, for backward compatibility)
 */
export interface LegacyMCPServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

/**
 * Top-level MCP configuration supporting both V1 and V2 formats
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfigV2 | LegacyMCPServerConfig>;
}

/**
 * Type guard to check if config is V2 format
 */
export function isMCPServerConfigV2(
  config: MCPServerConfigV2 | LegacyMCPServerConfig,
): config is MCPServerConfigV2 {
  return 'transport' in config && typeof config.transport === 'object';
}

/**
 * Convert legacy config to V2 format
 */
export function convertLegacyToV2(
  name: string,
  legacy: LegacyMCPServerConfig,
): MCPServerConfigV2 {
  return {
    name,
    transport: {
      type: 'stdio',
      command: legacy.command,
      args: legacy.args,
      env: legacy.env,
    },
  };
}
