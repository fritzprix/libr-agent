import { getLogger } from '../../logger';

const logger = getLogger('AnthropicToolInput');

export function parseAnthropicToolInput(
  raw: unknown,
  context: { messageId?: string; toolId?: string; toolName?: string },
): Record<string, unknown> {
  if (raw == null) {
    logger.warn('Tool call input missing; defaulting to empty object', context);
    return {};
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw);
      if (
        typeof parsed === 'object' &&
        parsed !== null &&
        !Array.isArray(parsed)
      ) {
        return parsed as Record<string, unknown>;
      }
      logger.error('Parsed tool call arguments must be an object', {
        ...context,
        parsedType: typeof parsed,
      });
      throw new Error('Parsed tool call arguments must be an object');
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

export function ensureAnthropicObjectInput(
  raw: unknown,
  context: { messageId?: string; toolId?: string; toolName?: string },
): Record<string, unknown> {
  if (raw == null) {
    return {};
  }

  if (typeof raw === 'object' && !Array.isArray(raw)) {
    return raw as Record<string, unknown>;
  }

  return parseAnthropicToolInput(raw, context);
}
