/**
 * 🏗️ MCP (Model Context Protocol) Type Definitions
 *
 * 이 파일은 MCP 프로토콜의 모든 타입 정의를 중앙집중화합니다.
 * MCP 사양을 준수하며, 모든 다른 파일에서 이 타입들을 import해서 사용합니다.
 *
 * 참조: https://modelcontextprotocol.io/
 */

import { UIResource } from '@mcp-ui/server';

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
  serviceInfo?: ServiceInfo;
}

export interface MCPImageContent {
  type: 'image';
  data: string; // base64
  mimeType: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

export interface MCPAudioContent {
  type: 'audio';
  data: string; // base64
  mimeType: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

export interface MCPResourceLinkContent {
  type: 'resource_link';
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

// 통합된 Resource content type (기존 두 타입을 하나로 병합)
type MCPResourceContent = UIResource & {
  serviceInfo?: ServiceInfo;
};

export type MCPContent =
  | MCPTextContent
  | MCPImageContent
  | MCPAudioContent
  | MCPResourceLinkContent
  | MCPResourceContent;

// ========================================
// 🔧 Service Context Types (for tool resolution)
// ========================================

export interface ServiceInfo {
  serverName: string;
  toolName: string;
  backendType: 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';
}

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
// 🔄 MCP Protocol Types (JSON-RPC 2.0 준수)
// ========================================

// Sampling 관련 타입 추가
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

export interface SamplingRequest {
  prompt: string;
  options?: SamplingOptions;
}

export interface MCPResult {
  content?: MCPContent[];
  structuredContent?: Record<string, unknown>;
  isError?: boolean; // MCP 표준: 도구 실행 에러 플래그
}

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

export interface MCPError {
  code: number;
  message: string;
  data?: unknown;
}

export interface SamplingResponse extends MCPResponse {
  result?: SamplingResult;
}

/**
 * 표준 MCP 응답 (JSON-RPC 2.0 사양 준수)
 * 모든 MCP 응답은 이 형식을 따라야 합니다.
 */
export interface MCPResponse {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult | SamplingResult;
  error?: MCPError;
}

/**
 * Extended MCP Response with service context information
 * Service context를 보존하여 UI에서 정확한 tool 재호출을 지원
 */
export interface ExtendedMCPResponse extends MCPResponse {
  serviceInfo?: {
    serverName: string;
    toolName: string;
    backendType: 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';
  };
}

/**
 * Check if response is ExtendedMCPResponse (type guard)
 */
export function isExtendedResponse(
  response: MCPResponse,
): response is ExtendedMCPResponse {
  return response && typeof response === 'object' && 'serviceInfo' in response;
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
  backend?: 'tauri' | 'webworker'; // 도구가 실행되는 백엔드
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
 * MCP 응답이 성공인지 확인 (타입 가드)
 */
export function isMCPSuccess(
  response: MCPResponse,
): response is MCPResponse & { result: MCPResult } {
  return response.error === undefined && response.result !== undefined;
}

/**
 * MCP 응답이 에러인지 확인 (타입 가드)
 */
export function isMCPError(
  response: MCPResponse,
): response is MCPResponse & { error: MCPError } {
  return response.error !== undefined;
}

/**
 * MCPResult에 유효한 content가 있는지 확인
 */
export function isValidMCPResult(result: MCPResult): boolean {
  return !!(result.content?.length || result.structuredContent);
}

/**
 * MCP 응답을 채팅 시스템용 문자열로 변환
 */
export function mcpResponseToString(response: MCPResponse): string {
  if (isMCPError(response)) {
    return JSON.stringify({
      error: response.error.message,
      success: false,
    });
  }

  if (isMCPSuccess(response) && response.result) {
    if (response.result.content) {
      const textContent = response.result.content
        .filter((c) => c.type === 'text')
        .map((c) => (c as MCPTextContent).text)
        .join('\n');

      // content에 에러가 포함되어 있는지 확인
      if (
        textContent &&
        (textContent.includes('"error":') ||
          textContent.includes('\\"error\\":'))
      ) {
        try {
          const parsed = JSON.parse(textContent);
          if (parsed.error) {
            return JSON.stringify({
              error: parsed.error,
              success: false,
            });
          }
        } catch {
          // JSON 파싱 실패 시 원본 반환하지만 에러로 표시
          return JSON.stringify({
            error: textContent,
            success: false,
          });
        }
      }

      if (textContent) return textContent;
    }
    if (response.result.structuredContent) {
      return JSON.stringify(response.result.structuredContent);
    }
    // content 와 structuredContent 모두 없는 경우
    return JSON.stringify(response.result);
  }

  // 이론적으로 도달하면 안 되는 경로
  return JSON.stringify({
    error: 'Invalid MCP Response structure',
    success: false,
  });
}

// ========================================
// 🌐 Web Worker MCP Types
// ========================================

/**
 * Web Worker MCP 서버 인터페이스
 */
export interface WebMCPServer {
  name: string;
  description?: string;
  version?: string;
  tools: MCPTool[];
  callTool: (name: string, args: unknown) => Promise<MCPResponse>;
  sampleText?: (
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
  getServiceContext?: () => Promise<string>;
}

/**
 * Web Worker MCP 메시지 타입
 */
export interface WebMCPMessage {
  id: string;
  type:
    | 'listTools'
    | 'callTool'
    | 'ping'
    | 'loadServer'
    | 'sampleText'
    | 'getServiceContext';
  serverName?: string;
  toolName?: string;
  args?: unknown;
}

/**
 * Web Worker MCP 프록시 설정
 */
export interface WebMCPProxyConfig {
  workerPath?: string;
  workerInstance?: Worker;
  timeout?: number;
  retryOptions?: {
    maxRetries?: number;
    baseDelay?: number;
    maxDelay?: number;
    timeout?: number;
  };
}

/**
 * Web Worker MCP 서버 상태
 */
export interface WebMCPServerState {
  loaded: boolean;
  tools: MCPTool[];
  lastError?: string;
  lastActivity?: number;
}

// ========================================
// 🔄 Unified MCP Types (Tauri + Web Worker)
// ========================================

/**
 * MCP 서버 타입 (Tauri 또는 Web Worker)
 */
export type MCPServerType = 'tauri' | 'webworker';

/**
 * 통합 MCP 서버 설정
 */
export interface UnifiedMCPServerConfig {
  name: string;
  type: MCPServerType;
  // Tauri 서버용
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  transport?: 'stdio' | 'http' | 'websocket';
  url?: string;
  port?: number;
  // Web Worker 서버용
  modulePath?: string;
  workerPath?: string;
}

/**
 * 통합 MCP 도구 실행 컨텍스트
 */
export interface MCPToolExecutionContext {
  serverType: MCPServerType;
  serverName: string;
  toolName: string;
  arguments: unknown;
  timeout?: number;
}
