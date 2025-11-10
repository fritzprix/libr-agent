/**
 * @file Service Info Utilities
 * @description Helper functions for extracting service information
 */

import type { MCPContent, ServiceInfo } from '../protocol';

/**
 * A type guard to check if an MCPContent object has service information.
 * @param content The content object to check.
 * @returns True if the content has service information, false otherwise.
 */
export function hasServiceInfo(
  content: MCPContent,
): content is MCPContent & { serviceInfo: ServiceInfo } {
  return (
    content &&
    typeof content === 'object' &&
    'serviceInfo' in content &&
    content.serviceInfo !== undefined
  );
}

/**
 * Extracts the first `ServiceInfo` object found in an array of MCP content parts.
 * @param content An array of MCPContent objects.
 * @returns The first `ServiceInfo` object found, or null if none exist.
 */
export function extractServiceInfoFromContent(
  content: MCPContent[],
): ServiceInfo | null {
  for (const item of content) {
    if (hasServiceInfo(item)) {
      return item.serviceInfo;
    }
  }
  return null;
}
