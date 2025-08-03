/**
 * 🏗️ MCP (Model Context Protocol) Type Definitions
 *
 * 이 파일은 MCP 프로토콜의 모든 타입 정의를 중앙집중화합니다.
 * MCP 사양을 준수하며, 모든 다른 파일에서 이 타입들을 import해서 사용합니다.
 *
 * 참조: https://modelcontextprotocol.io/
 */

// ========================================
// 🔧 JSON Schema Types (MCP 사양 준수)
// ========================================

export type JSONSchemaType =
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'object'
  | 'array'
  | 'null';

export interface JSONSchemaBase {
  type?: JSONSchemaType | JSONSchemaType[];
  title?: string;
  description?: string;
  default?: unknown;
  examples?: unknown[];
  enum?: unknown[];
  const?: unknown;
}

export interface JSONSchemaString extends JSONSchemaBase {
  type: 'string';
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
}

export interface JSONSchemaNumber extends JSONSchemaBase {
  type: 'number' | 'integer';
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}

export interface JSONSchemaBoolean extends JSONSchemaBase {
  type: 'boolean';
}

export interface JSONSchemaNull extends JSONSchemaBase {
  type: 'null';
}

export interface JSONSchemaArray extends JSONSchemaBase {
  type: 'array';
  items?: JSONSchema | JSONSchema[];
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
  additionalItems?: boolean | JSONSchema;
}

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

export type JSONSchema =
  | JSONSchemaString
  | JSONSchemaNumber
  | JSONSchemaBoolean
  | JSONSchemaNull
  | JSONSchemaArray
  | JSONSchemaObject
  | (JSONSchemaBase & { type?: JSONSchemaType | JSONSchemaType[] });

// ========================================
// 📄 MCP Content Types (사양 준수)
// ========================================

export interface MCPTextContent {
  type: 'text';
  text: string;
  annotations?: Record<string, unknown>;
}

export interface MCPImageContent {
  type: 'image';
  data: string; // base64
  mimeType: string;
  annotations?: Record<string, unknown>;
}

export interface MCPAudioContent {
  type: 'audio';
  data: string; // base64
  mimeType: string;
  annotations?: Record<string, unknown>;
}

export interface MCPResourceLinkContent {
  type: 'resource_link';
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  annotations?: Record<string, unknown>;
}

export interface MCPResourceContent {
  type: 'resource';
  resource: {
    uri: string;
    title?: string;
    mimeType?: string;
    text?: string;
    annotations?: Record<string, unknown>;
  };
}

export type MCPContent =
  | MCPTextContent
  | MCPImageContent
  | MCPAudioContent
  | MCPResourceLinkContent
  | MCPResourceContent;

// ========================================
// 🔄 MCP Protocol Types (JSON-RPC 2.0 준수)
// ========================================

export interface MCPResult {
  content?: MCPContent[];
  structuredContent?: Record<string, unknown>;
}

export interface MCPError {
  code: number;
  message: string;
  data?: unknown;
}

/**
 * 표준 MCP 응답 (JSON-RPC 2.0 사양 준수)
 * 모든 MCP 응답은 이 형식을 따라야 합니다.
 */
// MCP Response types (JSON-RPC 2.0 compliant)
export interface MCPResponse {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult;
  error?: MCPError;
}

// Helper functions to check response status
export function isSuccessResponse(response: MCPResponse): boolean {
  return response.error === undefined;
}

export function isErrorResponse(response: MCPResponse): boolean {
  return response.error !== undefined;
}

// ========================================
// 🛠️ MCP Tool Types
// ========================================

export interface MCPToolAnnotations {
  audience?: ('user' | 'assistant')[];
  priority?: number;
  lastModified?: string;
  [key: string]: unknown;
}

export interface MCPTool {
  name: string;
  title?: string;
  description: string;
  inputSchema: JSONSchemaObject;
  outputSchema?: JSONSchemaObject;
  annotations?: MCPToolAnnotations;
}

// ========================================
// 🔧 Server Configuration
// ========================================

export interface MCPServerConfig {
  name: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  transport: 'stdio' | 'http' | 'websocket';
  url?: string;
  port?: number;
}

// ========================================
// 🔀 Tool Call Result Types (Rust backend interface)
// ========================================

// Tool Call Result type (used by Rust backend via Tauri)
export interface ToolCallResult {
  success: boolean;
  result?: unknown;
  error?: string;
}

// ========================================
// 🔀 Legacy Support Types (점진적 마이그레이션용)
// ========================================

/**
 * @deprecated 레거시 지원용. 새 코드에서는 MCPResponse 사용
 */
export interface LegacyToolCallResult {
  success: boolean;
  result?: unknown;
  error?: string;
  isError?: boolean;
}

// ========================================
// 🎯 Helper Functions
// ========================================

/**
 * JSON Schema 생성 helper 함수들
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

/**
 * MCP 응답이 성공인지 확인
 */
export function isMCPSuccess(response: MCPResponse): boolean {
  return !response.error && response.result !== undefined;
}

/**
 * MCP 응답이 에러인지 확인
 */
export function isMCPError(response: MCPResponse): boolean {
  return response.error !== undefined;
}

/**
 * 레거시 응답을 MCP 형식으로 변환
 */
export function normalizeLegacyResponse(
  legacy: LegacyToolCallResult,
  toolName: string,
): MCPResponse {
  const id = `tool-${toolName}-${Date.now()}`;

  if (!legacy.success || legacy.error || legacy.isError) {
    return {
      jsonrpc: '2.0',
      id,
      error: {
        code: -32603,
        message: legacy.error || `Tool execution failed: ${toolName}`,
        data: legacy,
      },
    };
  }

  return {
    jsonrpc: '2.0',
    id,
    result: {
      content: [
        {
          type: 'text',
          text:
            typeof legacy.result === 'string'
              ? legacy.result
              : JSON.stringify(legacy.result || { success: true }),
        },
      ],
    },
  };
}

/**
 * MCP 응답을 채팅 시스템용 문자열로 변환
 */
export function mcpResponseToString(response: MCPResponse): string {
  if (response.error) {
    return JSON.stringify({
      error: response.error.message,
      success: false,
    });
  }

  if (response.result?.content) {
    const textContent = response.result.content
      .filter((c) => c.type === 'text')
      .map((c) => (c as MCPTextContent).text)
      .join('\n');

    return textContent || JSON.stringify(response.result);
  }

  if (response.result?.structuredContent) {
    return JSON.stringify(response.result.structuredContent);
  }

  return JSON.stringify(response.result || { success: true });
}
