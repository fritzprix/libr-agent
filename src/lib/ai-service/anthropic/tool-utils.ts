import { getLogger } from '../../logger';

const logger = getLogger('AnthropicToolUtils');

export function parseToolInput(
  raw: unknown,
  context: { messageId?: string; toolId?: string; toolName?: string },
): Record<string, unknown> {
  if (raw == null) {
    logger.warn('Tool call input missing; defaulting to empty object', context);
    return {};
  }

  if (typeof raw === 'string') {
    try {
      return JSON.parse(raw) as Record<string, unknown>;
    } catch (error) {
      logger.error('Failed to parse tool call arguments as JSON', {
        ...context,
        error,
      });
      throw error instanceof Error ? error : new Error(String(error));
    }
  }

  if (typeof raw === 'object' && !Array.isArray(raw)) {
    return raw as Record<string, unknown>;
  }

  logger.error('Unsupported tool call argument type', {
    ...context,
    valueType: typeof raw,
  });
  throw new Error('Unsupported tool call argument type');
}

export function ensureObjectInput(
  raw: unknown,
  context: { messageId?: string; toolId?: string; toolName?: string },
): Record<string, unknown> {
  if (raw == null) {
    return {};
  }

  if (typeof raw === 'object' && !Array.isArray(raw)) {
    return raw as Record<string, unknown>;
  }

  return parseToolInput(raw, context);
}
