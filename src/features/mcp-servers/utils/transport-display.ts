import type { ServerMetadata } from '@/lib/mcp/config/server-config';
import type { TransportConfig } from '@/lib/mcp/config/transport';

/**
 * Strip preset url-param secrets from an HTTP transport URL for card display.
 */
export function sanitizeHttpDisplayUrl(
  urlString: string,
  metadata?: ServerMetadata,
): string {
  try {
    const urlObj = new URL(urlString);
    const varDefs = metadata?.variableDefinitions;
    if (varDefs) {
      for (const [key, def] of Object.entries(varDefs)) {
        if (def.target === 'url-param' && urlObj.searchParams.has(key)) {
          urlObj.searchParams.delete(key);
        }
      }
    }
    return urlObj.toString();
  } catch {
    return urlString;
  }
}

/**
 * Command or URL shown after the transport type on ServerCard.
 */
export function getTransportEndpointLabel(
  transport: TransportConfig,
  metadata?: ServerMetadata,
): string | null {
  if (transport.type === 'stdio') {
    return transport.command;
  }
  if ('url' in transport) {
    return sanitizeHttpDisplayUrl(transport.url, metadata);
  }
  return null;
}
