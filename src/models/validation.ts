import { z } from 'zod';
import { Assistant } from './chat';

// Schema for the raw DTO returned by the backend
export const AssistantDtoSchema = z.object({
  id: z.string(),
  name: z.string(),
  config: z.unknown(), // JSON config value
  createdAt: z.number(), // Timestamp in ms
  updatedAt: z.number(), // Timestamp in ms
});

// Schema for the config part (stored as JSON in DB)
// This matches the fields in Assistant interface that are stored in config
const AssistantConfigSchema = z.object({
  description: z.string().optional(),
  avatar: z.string().optional(),
  systemPrompt: z.string().default(''), // Ensure default if missing
  mcpServerIds: z.array(z.string()).optional(),
  localServices: z.array(z.string()).optional(),
  allowedBuiltInServiceAliases: z.array(z.string()).optional(),
  deletionProtected: z.boolean().default(false),
  model: z.string().optional(),
  provider: z.string().optional(),
  temperature: z.number().optional(),
  maxTokens: z.number().optional(),
});

// Schema for the full Domain Model (after parsing)
// We use this to validate the final object structure
export const AssistantSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  avatar: z.string().optional(),
  systemPrompt: z.string(),
  mcpServerIds: z.array(z.string()).optional(),
  localServices: z.array(z.string()).optional(),
  allowedBuiltInServiceAliases: z.array(z.string()).optional(),
  deletionProtected: z.boolean(),
  model: z.string().optional(),
  provider: z.string().optional(),
  temperature: z.number().optional(),
  maxTokens: z.number().optional(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

/**
 * Safely parses and transforms backend Assistant DTO to Domain Model
 * Validates structure, flattens config, and converts timestamps to Dates.
 */
export function parseAssistant(data: unknown): Assistant {
  // 1. Validate the raw DTO structure
  const dto = AssistantDtoSchema.parse(data);

  // 2. Extract and validate config
  // Config can be:
  // - A JSON object (if parsed by serde before returning, which Rust does for Value)
  // - Null/Undefined (handled by optional/default in schema)
  let configParsed = {};

  if (dto.config && typeof dto.config === 'object') {
    // Safe parse the config part
    // We use safeParse to avoid blowing up if config is slightly malformed,
    // but for strictness, maybe we should throw?
    // Aegis says: "Ambiguity is the root of all runtime errors".
    // So we should enforce valid config.
    configParsed = AssistantConfigSchema.parse(dto.config);
  } else {
    // If config is missing or not an object, fallback to defaults
    configParsed = AssistantConfigSchema.parse({});
  }

  // 3. Construct the final object
  const assistant: Assistant = {
    id: dto.id,
    name: dto.name,
    ...configParsed,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  } as Assistant; // Type assertion needed because spread types are complex, but we validate below

  // 4. Final validation against the domain schema to be absolutely sure
  return AssistantSchema.parse(assistant);
}
