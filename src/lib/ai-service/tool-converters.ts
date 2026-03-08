import { ChatCompletionTool as GroqChatCompletionTool } from 'groq-sdk/resources/chat/completions.mjs';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { Tool as AnthropicTool } from '@anthropic-ai/sdk/resources/messages.mjs';
import { MCPTool, JSONSchema } from '@/lib/mcp';
import { getLogger } from '../logger';
import {
  FunctionDeclaration,
  Type,
  Schema as GeminiSchema,
} from '@google/genai';
import Cerebras from '@cerebras/cerebras_cloud_sdk';

const logger = getLogger('AIService');

// --- Tool Conversion with Enhanced Type Safety ---

/** A union type representing any possible provider-specific tool format. @internal */
type ProviderToolType =
  | GroqChatCompletionTool
  | OpenAIChatCompletionTool
  | AnthropicTool
  | FunctionDeclaration
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool
  | OllamaTool;

/** Represents the structure of a tool for the Ollama provider. @internal */
interface OllamaTool {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

/** Represents an array of provider-specific tools. @internal */
export type ProviderToolsType = ProviderToolType[];

/**
 * Recursively converts an MCPTool's JSONSchema into the Google GenAI FunctionDeclaration format.
 * This properly handles nested objects and arrays, preserving all schema information.
 * @param schema The MCP JSONSchema to convert.
 * @returns The schema in the format required by Google GenAI SDK.
 * @internal
 */
function convertMCPSchemaToGeminiParameters(schema: JSONSchema): GeminiSchema {
  // Base case: String type with optional enum
  if (schema.type === 'string') {
    const result: GeminiSchema = { type: Type.STRING };
    if (schema.description) result.description = schema.description;
    if ('enum' in schema && Array.isArray(schema.enum)) {
      result.enum = schema.enum as string[];
    }
    return result;
  }

  // Base case: Number types
  if (schema.type === 'number' || schema.type === 'integer') {
    const result: GeminiSchema = { type: Type.NUMBER };
    if (schema.description) result.description = schema.description;
    return result;
  }

  // Base case: Boolean type
  if (schema.type === 'boolean') {
    const result: GeminiSchema = { type: Type.BOOLEAN };
    if (schema.description) result.description = schema.description;
    return result;
  }

  // Base case: Null type
  if (schema.type === 'null') {
    const result: GeminiSchema = { type: Type.STRING };
    if (schema.description) result.description = schema.description;
    return result;
  }

  // Recursive case: Arrays
  if (schema.type === 'array' && 'items' in schema && schema.items) {
    const arrayItems = Array.isArray(schema.items)
      ? schema.items[0]
      : schema.items;
    const result: GeminiSchema = {
      type: Type.ARRAY,
      items: arrayItems
        ? convertMCPSchemaToGeminiParameters(arrayItems)
        : { type: Type.STRING },
    };
    if (schema.description) result.description = schema.description;
    return result;
  }

  // Recursive case: Objects
  if (schema.type === 'object' && 'properties' in schema && schema.properties) {
    const geminiProperties: Record<string, GeminiSchema> = {};

    for (const [key, propSchema] of Object.entries(schema.properties)) {
      geminiProperties[key] = convertMCPSchemaToGeminiParameters(propSchema);
    }

    const result: GeminiSchema = {
      type: Type.OBJECT,
      properties: geminiProperties,
    };
    if (schema.description) result.description = schema.description;
    return result;
  }

  // Fallback for unknown or incomplete types
  return { type: Type.STRING };
}

/**
 * Ensures that a JSON schema and its nested properties have a `type` field,
 * which is required by some providers. It infers the type if it's missing.
 * @param schema The schema to process.
 * @returns A new schema object with the `type` field ensured.
 * @internal
 */
function ensureSchemaTypeField(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  if (!schema || typeof schema !== 'object') {
    return { type: 'object', properties: {} };
  }

  const result = { ...schema };

  // Ensure root schema has type field
  if (!result.type) {
    // Infer type from structure
    if (result.properties && typeof result.properties === 'object') {
      result.type = 'object';
    } else if (result.items) {
      result.type = 'array';
    } else {
      result.type = 'object'; // default fallback
    }
  }

  // Handle array-type type fields (convert to single type)
  if (Array.isArray(result.type)) {
    // Prioritize the first non-null type
    const nonNullType = (result.type as string[]).find((t) => t !== 'null');
    result.type = nonNullType || 'string';
  }

  // Recursively ensure properties have type fields
  if (result.properties && typeof result.properties === 'object') {
    const properties = result.properties as Record<string, unknown>;
    const fixedProperties: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(properties)) {
      if (typeof value === 'object' && value !== null) {
        fixedProperties[key] = ensureSchemaTypeField(
          value as Record<string, unknown>,
        );
      } else {
        fixedProperties[key] = value;
      }
    }
    result.properties = fixedProperties;
  }

  // Recursively ensure array items have type fields
  if (result.items) {
    if (Array.isArray(result.items)) {
      result.items = result.items.map((item) =>
        typeof item === 'object' && item !== null
          ? ensureSchemaTypeField(item as Record<string, unknown>)
          : item,
      );
    } else if (typeof result.items === 'object' && result.items !== null) {
      result.items = ensureSchemaTypeField(
        result.items as Record<string, unknown>,
      );
    }
  }

  return result;
}

/**
 * Sanitizes a JSON schema for Cerebras compatibility by removing unsupported fields.
 * @param schema The schema to sanitize.
 * @returns A new, sanitized schema object.
 * @internal
 */
function sanitizeSchemaForCerebras(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  if (!schema || typeof schema !== 'object') {
    return { type: 'object', properties: {} };
  }

  // First ensure type field is present
  const schemaWithType = ensureSchemaTypeField(schema);
  const sanitized = { ...schemaWithType };

  // Remove unsupported fields at the root level (based on Cerebras error feedback)
  const unsupportedFields = [
    'minimum',
    'maximum',
    'exclusiveMinimum',
    'exclusiveMaximum',
    'multipleOf',
    'pattern',
    'format',
  ];
  unsupportedFields.forEach((field) => {
    delete sanitized[field];
  });

  // Recursively sanitize properties
  if (sanitized.properties && typeof sanitized.properties === 'object') {
    const originalProperties = sanitized.properties as Record<string, unknown>;
    const sanitizedProperties: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(originalProperties)) {
      if (typeof value === 'object' && value !== null) {
        sanitizedProperties[key] = sanitizeSchemaForCerebras(
          value as Record<string, unknown>,
        );
      } else {
        sanitizedProperties[key] = value;
      }
    }

    sanitized.properties = sanitizedProperties;
  }

  // Recursively sanitize array items
  if (sanitized.items) {
    if (Array.isArray(sanitized.items)) {
      sanitized.items = sanitized.items.map((item) =>
        typeof item === 'object' && item !== null
          ? sanitizeSchemaForCerebras(item as Record<string, unknown>)
          : item,
      );
    } else if (
      typeof sanitized.items === 'object' &&
      sanitized.items !== null
    ) {
      sanitized.items = sanitizeSchemaForCerebras(
        sanitized.items as Record<string, unknown>,
      );
    }
  }

  // Ensure object schemas have valid structure as required by Cerebras
  if (sanitized.type === 'object') {
    const hasProperties =
      sanitized.properties &&
      typeof sanitized.properties === 'object' &&
      Object.keys(sanitized.properties).length > 0;
    const hasAnyOf = sanitized.anyOf && Array.isArray(sanitized.anyOf);

    if (!hasProperties && !hasAnyOf) {
      // Provide minimal valid object schema
      sanitized.properties = {};
    }
    // Cerebras requires additionalProperties to be false
    sanitized.additionalProperties = false;
  }

  return sanitized;
}

/**
 * Validates the basic structure of an `MCPTool`.
 * @param tool The tool to validate.
 * @throws An error if the tool is missing required fields.
 * @internal
 */
function validateTool(tool: MCPTool): void {
  if (!tool.name || typeof tool.name !== 'string') {
    throw new Error('Tool must have a valid name');
  }
  if (!tool.description || typeof tool.description !== 'string') {
    throw new Error('Tool must have a valid description');
  }
  if (!tool.inputSchema || typeof tool.inputSchema !== 'object') {
    throw new Error('Tool must have a valid inputSchema');
  }
  if (tool.inputSchema.type !== 'object') {
    throw new Error('Tool inputSchema must be of type "object"');
  }
}

/**
 * Converts a single MCPTool to the Cerebras-specific format.
 * Cerebras requires sanitized JSON schemas without unsupported fields.
 * @param mcpTool The MCPTool to convert.
 * @returns The tool in Cerebras-specific format.
 * @internal
 */
export function convertMCPToolToCerebras(
  mcpTool: MCPTool,
): Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool {
  validateTool(mcpTool);

  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = ensureSchemaTypeField({
    type: 'object' as const,
    properties: properties,
    required: required,
  });

  const sanitizedParameters = sanitizeSchemaForCerebras(commonParameters);

  logger.info('Cerebras tool conversion:', {
    toolName: mcpTool.name,
  });

  return {
    type: 'function',
    function: {
      name: mcpTool.name,
      description: mcpTool.description,
      parameters: sanitizedParameters,
    },
  } satisfies Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool;
}

/**
 * Converts a single MCPTool to the OpenAI-compatible format.
 * Used by OpenAI and Fireworks providers.
 * @param mcpTool The MCPTool to convert.
 * @returns The tool in OpenAI-compatible format.
 * @internal
 */
export function convertMCPToolToOpenAI(
  mcpTool: MCPTool,
): OpenAIChatCompletionTool {
  validateTool(mcpTool);

  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = ensureSchemaTypeField({
    type: 'object' as const,
    properties: properties,
    required: required,
  });

  return {
    type: 'function',
    function: {
      name: mcpTool.name,
      description: mcpTool.description,
      parameters: commonParameters,
    },
  } satisfies OpenAIChatCompletionTool;
}

/**
 * Converts a single MCPTool to the Groq-specific format.
 * @param mcpTool The MCPTool to convert.
 * @returns The tool in Groq-specific format.
 * @internal
 */
export function convertMCPToolToGroq(mcpTool: MCPTool): GroqChatCompletionTool {
  validateTool(mcpTool);

  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = ensureSchemaTypeField({
    type: 'object' as const,
    properties: properties,
    required: required,
  });

  return {
    type: 'function',
    function: {
      name: mcpTool.name,
      description: mcpTool.description,
      parameters: commonParameters,
    },
  };
}

/**
 * Converts a single MCPTool to the Anthropic-specific format.
 * @param mcpTool The MCPTool to convert.
 * @returns The tool in Anthropic-specific format.
 * @internal
 */
export function convertMCPToolToAnthropic(mcpTool: MCPTool): AnthropicTool {
  validateTool(mcpTool);

  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = ensureSchemaTypeField({
    type: 'object' as const,
    properties: properties,
    required: required,
  });

  return {
    name: mcpTool.name,
    description: mcpTool.description,
    input_schema: commonParameters as AnthropicTool['input_schema'],
  };
}

/**
 * Converts a single MCPTool to the Gemini-specific format.
 * @param mcpTool The MCPTool to convert.
 * @returns The tool in Gemini-specific format.
 * @internal
 */
export function convertMCPToolToGemini(mcpTool: MCPTool): FunctionDeclaration {
  validateTool(mcpTool);

  // Convert the entire inputSchema to Gemini format
  const geminiParams = convertMCPSchemaToGeminiParameters(mcpTool.inputSchema);

  return {
    name: mcpTool.name,
    description: mcpTool.description,
    parameters: {
      type: Type.OBJECT,
      description: mcpTool.inputSchema.description,
      properties: geminiParams.properties || {},
      required: mcpTool.inputSchema.required || [],
    },
  } satisfies FunctionDeclaration;
}
