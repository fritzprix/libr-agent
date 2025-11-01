/**
 * @file Web Worker MCP Message Types
 * @description Message protocol for Web Worker communication
 */

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
