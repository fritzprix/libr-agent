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
  | MCPImageContent
  | MCPAudioContent
  | MCPResourceLinkContent
  | MCPResourceContent;
