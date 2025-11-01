/**
 * @file MCP Transport Configuration Types
 * @description Transport and authentication configuration (MCP 2025-06-18 Spec)
 */

/**
 * Discriminated union for transport-specific configurations
 */
export type TransportConfig =
  | {
      type: 'stdio';
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  | {
      type: 'http';
      url: string;
      protocolVersion?: string; // Default: "2025-06-18"
      sessionId?: string;
      headers?: Record<string, string>;
      enableSSE?: boolean; // For backward compatibility with older servers
      security?: {
        enableDnsRebindingProtection?: boolean;
        allowedOrigins?: string[];
        allowedHosts?: string[];
      };
    };

/**
 * OAuth 2.1 authentication configuration (RFC 8414, RFC 7636)
 */
export interface OAuthConfig {
  type: 'oauth2.1';
  discoveryUrl?: string; // RFC 8414 Authorization Server Metadata
  authorizationEndpoint?: string; // Fallback if discovery not available
  tokenEndpoint?: string;
  registrationEndpoint?: string; // RFC 7591 Dynamic Client Registration
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
  usePKCE?: boolean; // Default: true
  resourceParameter?: string; // RFC 9728 Protected Resource Metadata
}
