import type { TransportConfig } from '@/lib/mcp/config/transport';

/**
 * Validates environment variables or headers
 * Throws error if value is a string (common AI mistake) to force correction
 */
export function validateRecordField(
  fieldName: string,
  value: unknown,
): Record<string, string> | undefined {
  if (!value) return undefined;

  // Check for string input (common AI mistake)
  if (typeof value === 'string') {
    throw new Error(
      `[Format Error] Invalid type for parameter '${fieldName}'.\n` +
        `Expected: Object (Record<string, string>)\n` +
        `Received: String (JSON string)\n\n` +
        `❌ Common Mistake: You provided a JSON string instead of a raw JSON object.\n` +
        `✅ Correct Usage:\n` +
        `   "${fieldName}": { "KEY": "VALUE" }\n\n` +
        `⛔ Incorrect Usage:\n` +
        `   "${fieldName}": "{\\"KEY\\": \\"VALUE\\"}"`,
    );
  }

  // Check for non-object input
  if (typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(
      `[Format Error] Invalid type for parameter '${fieldName}'.\n` +
        `Expected: Object\n` +
        `Received: ${Array.isArray(value) ? 'Array' : typeof value}\n` +
        `Please provide a key-value object.`,
    );
  }

  return value as Record<string, string>;
}

/**
 * Normalizes transport configuration to ensure proper types
 * Handles cases where AI agents provide JSON strings instead of objects
 */
export function normalizeTransportConfig(transport: unknown): TransportConfig {
  if (!transport || typeof transport !== 'object') {
    throw new Error('Transport configuration is required');
  }

  const t = transport as Record<string, unknown>;

  if (t.type === 'stdio') {
    if (!t.command || typeof t.command !== 'string') {
      throw new Error('Stdio transport requires a command string');
    }

    // Validate args if present
    if (t.args !== undefined) {
      if (typeof t.args === 'string') {
        throw new Error(
          `[Format Error] Invalid type for parameter 'args'.\n` +
            `Expected: Array of strings (e.g. ["arg1", "arg2"])\n` +
            `Received: String\n` +
            `❌ Common Mistake: You provided a JSON string instead of a raw Array.\n` +
            `✅ Correct Usage: "args": ["--flag", "value"]`,
        );
      }
      if (!Array.isArray(t.args)) {
        throw new Error(`[Format Error] 'args' must be an array of strings.`);
      }
    }

    return {
      type: 'stdio',
      command: t.command,
      args: t.args ? (t.args as unknown[]).map(String) : undefined,
      env: validateRecordField('env', t.env),
    };
  }

  if (t.type === 'http') {
    if (!t.url || typeof t.url !== 'string') {
      throw new Error('HTTP transport requires a url string');
    }

    return {
      type: 'http',
      url: t.url,
      protocolVersion:
        typeof t.protocolVersion === 'string' ? t.protocolVersion : undefined,
      sessionId: typeof t.sessionId === 'string' ? t.sessionId : undefined,
      headers: validateRecordField('headers', t.headers),
      enableSSE: typeof t.enableSSE === 'boolean' ? t.enableSSE : undefined,
      security:
        t.security && typeof t.security === 'object'
          ? (t.security as {
              enableDnsRebindingProtection?: boolean;
              allowedOrigins?: string[];
              allowedHosts?: string[];
            })
          : undefined,
    } as Extract<TransportConfig, { type: 'http' }>;
  }

  throw new Error(`Unsupported transport type: ${String(t.type)}`);
}
