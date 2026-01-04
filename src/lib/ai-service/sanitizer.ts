import type { Message, ToolCall } from '@/models/chat';

// JSON field safety verification and escape processing
export const sanitizeJsonField = (value: string): string => {
  try {
    JSON.parse(value);
    return value; // Return as is if valid JSON
  } catch {
    return JSON.stringify(value); // Convert to escaped string if malformed
  }
};

// ToolCall safety processing
export const sanitizeToolCall = (toolCall: ToolCall): ToolCall => {
  return {
    ...toolCall,
    function: {
      ...toolCall.function,
      arguments: sanitizeJsonField(toolCall.function.arguments),
    },
  };
};

// Message overall safety processing
export const sanitizeMessage = (message: Message): Message => {
  const sanitized = { ...message };

  // Process tool_calls
  if (sanitized.tool_calls) {
    sanitized.tool_calls = sanitized.tool_calls.map(sanitizeToolCall);
  }

  // Process thinking content
  if (sanitized.thinking) {
    sanitized.thinking = sanitizeJsonField(sanitized.thinking);
  }

  return sanitized;
};
