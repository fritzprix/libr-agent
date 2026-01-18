/**
 * @file Web Worker MCP Message Types
 * @description Message protocol for Web Worker communication
 */

import type { MCPResponse } from '../protocol/response';

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
    | 'getMetadata'
    | 'listServers';
  /** The name of the server, for loading specific servers. */
  serverName?: string;
  /** The name of the tool to call. */
  toolName?: string;
  /** The arguments for the tool call. */
  args?: unknown;
}

/**
 * Notification message sent from Worker to Main Thread
 * (asynchronous, no request ID required)
 */
export interface WebMCPNotification {
  /** Message type identifier */
  type: 'notify';
  /** Notification type (e.g., 'db-changed', 'server-status') */
  notifyType: string;
  /** Notification payload */
  data?: unknown;
}

/**
 * Combined message type for Worker → Main Thread communication
 */
export type WebMCPWorkerMessage = MCPResponse<unknown> | WebMCPNotification;
