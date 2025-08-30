/**
 * Content hash calculation utilities for browser and Node.js compatibility
 */

/**
 * Calculates a SHA-256 hash of the given content string
 * Compatible with both browser and Node.js environments
 */
export async function computeContentHash(content: string): Promise<string> {
  // Check if we're in a browser environment with Web Crypto API
  if (typeof crypto !== 'undefined' && crypto.subtle) {
    // Browser environment - use Web Crypto API
    const encoder = new TextEncoder();
    const data = encoder.encode(content);
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    const hashHex = hashArray
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
    return hashHex;
  }

  // Node.js environment - use crypto module
  try {
    const crypto = await import('node:crypto');
    const hash = crypto.createHash('sha256');
    hash.update(content, 'utf8');
    return hash.digest('hex');
  } catch {
    // Fallback to a simple hash function if crypto is not available
    // This is not cryptographically secure but provides basic deduplication
    return simpleHash(content);
  }
}

/**
 * Simple hash function as fallback
 * Not cryptographically secure but provides basic content deduplication
 */
function simpleHash(str: string): string {
  let hash = 0;
  if (str.length === 0) return hash.toString(16);

  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32bit integer
  }

  return Math.abs(hash).toString(16).padStart(8, '0');
}
