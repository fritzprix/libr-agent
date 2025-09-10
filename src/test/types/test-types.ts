/**
 * Type definitions for test utilities
 */

export interface MCPStructuredResponse {
  type: 'structured';
  text?: string;
  data: Record<string, unknown>;
  id: string;
}

export interface MCPErrorResponse {
  type: 'error';
  code: number;
  message: string;
  data: Record<string, unknown>;
  id: string;
}

export type MCPResponse = MCPStructuredResponse | MCPErrorResponse;

export interface ExtractContentToolData {
  content?: string;
  format?: string;
  raw_html_content?: string;
  save_html_requested?: boolean;
  metadata: {
    extraction_timestamp: string;
    content_length: number;
    raw_html_size: number;
    selector: string;
    format: string;
  };
  [key: string]: unknown;
}

export interface ValidationResult {
  isValid: boolean;
  messages: string[];
}

export interface PerformanceResult<T> {
  result: T;
  executionTime: number;
  withinLimit: boolean;
}

export interface ToolSchema {
  name: string;
  description: string;
  inputSchema: {
    type: string;
    properties: Record<string, unknown>;
    required?: string[];
  };
  execute: (
    args: Record<string, unknown>,
    executeScript?: (sessionId: string, script: string) => Promise<string>,
  ) => Promise<unknown>;
}

export interface JSONStructuredContent {
  tag: string;
  text?: string;
  children?: JSONStructuredContent[];
  id?: string;
  class?: string;
  href?: string;
  src?: string;
  alt?: string;
  title?: string;
  type?: string;
  placeholder?: string;
}

export interface JSONExtractionResult {
  title: string;
  url: string;
  timestamp: string;
  content: JSONStructuredContent;
}
