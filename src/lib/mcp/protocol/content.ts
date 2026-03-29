/**
 * @file MCP Content Types
 * @description Content types for MCP messages (text, image, audio, resources)
 * @see https://modelcontextprotocol.io/
 */

import { UIResource } from '@mcp-ui/server';

/**
 * Provides information about the service that generated a content part.
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
 * Represents a text content part in an MCP message.
 */
export interface MCPTextContent {
  type: 'text';
  text: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents an error content part in an MCP message.
 * Extended from MCPTextContent with an isError flag.
 */
export interface MCPErrorContent extends MCPTextContent {
  isError: true;
}

/**
 * Represents an image content part in an MCP message.
 */
export interface MCPImageContent {
  type: 'image';
  /** The image data encoded in base64. */
  data?: string;
  /** The image source URI. */
  uri?: string;
  /** Structured source descriptor for lazily materialized media. */
  source?: {
    data?: string;
    uri?: string;
    mimeType?: string;
  };
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
  data?: string;
  /** The audio source URI. */
  uri?: string;
  /** Structured source descriptor for lazily materialized media. */
  source?: {
    data?: string;
    uri?: string;
    mimeType?: string;
  };
  mimeType: string;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents a video content part in an MCP message.
 */
export interface MCPVideoContent {
  type: 'video';
  /** The video data encoded in base64. */
  data?: string;
  /** The video source URI. */
  uri?: string;
  /** Structured source descriptor for lazily materialized media. */
  source?: {
    data?: string;
    uri?: string;
    mimeType?: string;
  };
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
 * with optional service information.
 */
type MCPResourceContent = UIResource & {
  serviceInfo?: ServiceInfo;
};

/**
 * A union type representing any valid MCP content part.
 */
export type MCPContent =
  | MCPTextContent
  | MCPErrorContent
  | MCPImageContent
  | MCPAudioContent
  | MCPVideoContent
  | MCPResourceLinkContent
  | MCPResourceContent
  | MCPThinkingContent
  | MCPToolCallContent;

/**
 * Represents a thinking/reasoning content part.
 */
export interface MCPThinkingContent {
  type: 'thinking';
  thinking: string;
  signature?: string;
  thinkingTime?: number;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Represents a tool call within the content stream.
 */
export interface MCPToolCallContent {
  type: 'tool_call';
  id: string;
  name: string;
  arguments: string;
  isError?: boolean;
  annotations?: Record<string, unknown>;
  serviceInfo?: ServiceInfo;
}

/**
 * Type guard to check if content is an error content.
 * @param content - The content to check
 * @returns True if content is an MCPErrorContent
 */
export function isMCPErrorContent(
  content: MCPContent,
): content is MCPErrorContent {
  return (
    content.type === 'text' &&
    'isError' in content &&
    (content as MCPErrorContent).isError === true
  );
}
