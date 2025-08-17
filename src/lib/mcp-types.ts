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

// UIResource interface for MCP-UI integration (확장하여 MCP 표준과 호환)
export interface UIResource {
  uri?: string; // ui://... 형태 권장 (MCP 표준에서는 필수이지만 UI에서는 optional)
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  text?: string; // inline HTML or remote-dom script
  blob?: string; // base64-encoded content when used
  // MCP 표준 추가 필드 (UI에서는 optional)
  title?: string;
  annotations?: Record<string, unknown>;
}

// 통합된 Resource content type (기존 두 타입을 하나로 병합)
export interface MCPResourceContent {
  type: 'resource';
  resource: UIResource; // UIResource로 통일하여 UI와 표준 MCP 모두 지원
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
  isError?: boolean; // MCP 표준: 도구 실행 에러 플래그
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
export interface MCPResponse {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult;
  error?: MCPError;
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
 * 다양한 Tool 실행 결과를 일관된 MCPResponse 형식으로 변환
 * @param result Tool 실행 결과 (모든 타입 가능)
 * @param toolName Tool 이름
 * @returns MCPResponse 객체
 */
export function normalizeToolResult(
  result: unknown,
  toolName: string,
  deterministicId?: string,
): MCPResponse {
  // Use provided deterministic ID or fallback to tool name based ID
  const id =
    deterministicId ||
    `normalize-${toolName}-${Math.random().toString(36).slice(2)}`;

  // 1. 이미 MCPResponse 형식인 경우 그대로 반환
  if (
    typeof result === 'object' &&
    result !== null &&
    'jsonrpc' in result &&
    (result as MCPResponse).jsonrpc === '2.0'
  ) {
    return result as MCPResponse;
  }

  // 2. 에러 패턴 감지 (핵심 개선사항)
  // - 문자열에 'error' 포함
  // - 객체에 'error' 프로퍼티 포함
  // - 객체에 'success: false' 포함
  // - JSON 문자열 내부에 에러 포함 (error.txt 케이스 대응)
  const isError =
    (typeof result === 'string' &&
      (result.toLowerCase().includes('error') ||
        result.toLowerCase().includes('failed') ||
        result.includes('"error":') || // JSON 내부 에러 감지
        result.includes('\\"error\\":'))) || // 이스케이프된 JSON 내부 에러 감지
    (typeof result === 'object' &&
      result !== null &&
      ('error' in result ||
        ('success' in result && !(result as { success: boolean }).success)));

  if (isError) {
    // 에러 메시지 추출 로직 개선
    let errorMessage: string;

    if (typeof result === 'string') {
      // JSON 문자열인지 확인하고 파싱 시도
      if (result.includes('"error":') || result.includes('\\"error\\":')) {
        try {
          const parsed = JSON.parse(result);
          errorMessage = parsed.error || result;
        } catch {
          // JSON 파싱 실패 시 원본 문자열 사용
          errorMessage = result;
        }
      } else {
        errorMessage = result;
      }
    } else if (
      typeof result === 'object' &&
      result !== null &&
      'error' in result
    ) {
      errorMessage = String((result as { error: unknown }).error);
    } else {
      errorMessage = `Unknown error in ${toolName}`;
    }

    return {
      jsonrpc: '2.0',
      id,
      error: {
        code: -32603, // Internal error
        message: errorMessage,
        data: result,
      },
    };
  }

  // 3. 성공 결과 변환
  const textContent =
    typeof result === 'string' ? result : JSON.stringify(result, null, 2);

  return {
    jsonrpc: '2.0',
    id,
    result: {
      content: [
        {
          type: 'text',
          text: textContent,
        },
      ],
    },
  };
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
      return JSON.stringify(response.result.structuredContent, null, 2);
    }
    // content 와 structuredContent 모두 없는 경우
    return JSON.stringify(response.result, null, 2);
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
  callTool: (name: string, args: unknown) => Promise<unknown>;
}

/**
 * Web Worker MCP 메시지 타입
 */
export interface WebMCPMessage {
  id: string;
  type: 'listTools' | 'callTool' | 'ping' | 'loadServer';
  serverName?: string;
  toolName?: string;
  args?: unknown;
}

/**
 * Web Worker MCP 응답 타입 (간단한 형태)
 * 표준 MCPResponse와 완전히 동일한 구조를 사용하여 일관성 보장
 * Web Worker와 Tauri MCP 서버 간 응답 형식 통일
 */
export interface WebMCPResponse {
  id: string | number;
  result?: unknown;
  error?: string;
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

/**
 * 테스트용: error.txt와 같은 케이스를 검증하는 함수
 */
export function testErrorDetection(): void {
  // error.txt에서 발견된 케이스 테스트
  const errorCase = {
    success: true,
    content:
      '{\n  "error": "fieldSelector.replace is not a function",\n  "tool": "updateGame",\n  "timestamp": "2025-08-03T11:26:28.643Z"\n}',
    metadata: {
      toolName: 'rpg-server__updateGame',
      isValidated: true,
    },
    toolName: 'rpg-server__updateGame',
    executionTime: 97,
    timestamp: '2025-08-03T11:26:28.728Z',
  };

  const normalizedResponse = normalizeToolResult(
    errorCase.content,
    errorCase.toolName,
  );

  console.log('Test Result:', {
    original: errorCase,
    normalized: normalizedResponse,
    isError: isMCPError(normalizedResponse),
    isSuccess: isMCPSuccess(normalizedResponse),
  });
}

/**
 * Web Worker 응답을 표준 MCPResponse로 변환하는 헬퍼 함수
 * 레거시 WebMCPResponse 형식을 지원하기 위한 변환기
 */
export function normalizeWebMCPResponse(response: {
  id: string;
  result?: unknown;
  error?: string;
}): MCPResponse {
  const mcpResponse: MCPResponse = {
    jsonrpc: '2.0',
    id: response.id,
  };

  if (response.error) {
    mcpResponse.error = {
      code: -32603, // Internal error
      message: response.error,
    };
  } else if (response.result !== undefined) {
    // 결과를 표준 MCPResult 형식으로 변환
    if (typeof response.result === 'string') {
      mcpResponse.result = {
        content: [
          {
            type: 'text',
            text: response.result,
          },
        ],
      };
    } else {
      mcpResponse.result = {
        content: [
          {
            type: 'text',
            text: JSON.stringify(response.result, null, 2),
          },
        ],
      };
    }
  }

  return mcpResponse;
}
