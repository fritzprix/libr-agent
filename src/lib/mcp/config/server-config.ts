/**
 * @file MCP Server Configuration Types
 * @description Server configuration for both legacy and V2 formats
 */

import type { TransportConfig, OAuthConfig } from './transport';

/**
 * Server metadata (optional descriptive information)
 */
export interface ServerMetadata {
  category?: string;
  description?: string;
  logo?: string;
  vendor?: string;
  version?: string;
  variableDefinitions?: Record<
    string,
    {
      label?: string;
      description?: string;
      required?: boolean;
      type?: string;
      /** Where the value goes: env var (default), raw header, or Authorization Bearer token */
      target?: 'env' | 'header' | 'bearer-token' | 'url-param';
    }
  >;
}

/**
 * MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
 */
export interface MCPServerConfig {
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
}

/**
 * Top-level MCP configuration
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfig>;
}
