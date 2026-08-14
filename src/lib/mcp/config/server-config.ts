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
  /**
   * Origin of this server config.
   * `registry` = installed/edited from mcp-server.json presets.
   */
  source?: 'registry' | 'custom';
  /**
   * Meaningful preset defaults keyed by variableDefinition name.
   * Used to put prefilled fields under Advanced without per-MCP branching.
   * Empty / YOUR_* placeholders are omitted (those stay on the main form).
   */
  variableDefaults?: Record<string, string>;
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
