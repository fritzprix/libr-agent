import { z } from 'zod';
import { getLogger } from '@/lib/logger';
import type { Assistant, Message } from './chat';

const logger = getLogger('AssistantValidation');

// Schema for the nested config object
const AssistantConfigSchema = z
  .object({
    description: z.string().optional(),
    avatar: z.string().optional(),
    systemPrompt: z.string().default('You are a helpful assistant.'),
    mcpServerIds: z.array(z.string()).optional(),
    localServices: z.array(z.string()).optional(),
    allowedBuiltInServiceAliases: z.array(z.string()).optional(),
    deletionProtected: z.boolean().default(false),
    model: z.string().optional(),
    provider: z.string().optional(),
    temperature: z.number().optional(),
    maxTokens: z.number().optional(),
  })
  .passthrough();

type AssistantConfig = z.infer<typeof AssistantConfigSchema>;

// Schema for the raw DTO coming from backend
const AssistantDtoSchema = z.object({
  id: z.string(),
  name: z.string(),
  config: z.unknown(), // Can be an object or null
  createdAt: z.number(), // timestamp
  updatedAt: z.number(), // timestamp
});

export const parseAssistant = (data: unknown): Assistant => {
  const dto = AssistantDtoSchema.parse(data);

  const configParsed = AssistantConfigSchema.safeParse(dto.config);

  if (!configParsed.success && dto.config) {
    logger.warn('Assistant config validation failed', configParsed.error);
  }

  const config: Partial<AssistantConfig> = configParsed.success
    ? configParsed.data
    : {};

  return {
    id: dto.id,
    name: dto.name,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),

    // Spread config fields
    description: config.description,
    avatar: config.avatar,
    systemPrompt: config.systemPrompt ?? 'You are a helpful assistant.',
    mcpServerIds: config.mcpServerIds,
    localServices: config.localServices,
    allowedBuiltInServiceAliases: config.allowedBuiltInServiceAliases,
    deletionProtected: config.deletionProtected ?? false,
    model: config.model,
    provider: config.provider,
    temperature: config.temperature,
    maxTokens: config.maxTokens,
  };
};

/**
 * Type guard to validate if a partial message has all required fields to be treated as a full Message.
 * This is crucial for streaming messages which start as Partial<Message>.
 */
export const isValidMessage = (
  msg: Partial<Message> | undefined,
): msg is Message => {
  if (!msg) return false;
  return (
    typeof msg.id === 'string' &&
    typeof msg.sessionId === 'string' &&
    typeof msg.threadId === 'string' &&
    (msg.role === 'user' ||
      msg.role === 'assistant' ||
      msg.role === 'system' ||
      msg.role === 'tool') &&
    Array.isArray(msg.content)
  );
};
