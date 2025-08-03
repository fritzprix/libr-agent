import Cerebras from '@cerebras/cerebras_cloud_sdk';
import { FunctionDeclaration, Type } from '@google/genai';
import { ChatCompletionTool as GroqChatCompletionTool } from 'groq-sdk/resources/chat/completions.mjs';
import { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import { Tool as AnthropicTool } from '@anthropic-ai/sdk/resources/messages.mjs';
import { MCPTool, JSONSchema } from '../mcp-types';
import { getLogger } from '../logger';
import { AIServiceProvider, AIServiceError } from './types';

const logger = getLogger('AIService');

// --- Tool Conversion with Enhanced Type Safety ---

type ProviderToolType =
  | GroqChatCompletionTool
  | OpenAIChatCompletionTool
  | AnthropicTool
  | FunctionDeclaration
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool
  | OllamaTool;

interface OllamaTool {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}
type ProviderToolsType = ProviderToolType[];

// Helper function to convert JSON schema types to Gemini types
interface JsonSchemaProperty {
  type: string;
  description?: string;
  items?: JsonSchemaProperty;
}

function convertJSONSchemaToJsonSchemaProperty(
  schema: JSONSchema,
): JsonSchemaProperty {
  // Extract the base type from our structured JSONSchema
  const getTypeString = (schema: JSONSchema): string => {
    if (schema.type === 'string') return 'string';
    if (schema.type === 'number') return 'number';
    if (schema.type === 'integer') return 'integer';
    if (schema.type === 'boolean') return 'boolean';
    if (schema.type === 'array') return 'array';
    if (schema.type === 'object') return 'object';
    if (schema.type === 'null') return 'null';
    return 'string'; // fallback
  };

  const result: JsonSchemaProperty = {
    type: getTypeString(schema),
    description: schema.description,
  };

  // Handle array items recursively
  if (schema.type === 'array' && 'items' in schema && schema.items) {
    if (Array.isArray(schema.items)) {
      // If items is an array, use the first item
      result.items =
        schema.items.length > 0
          ? convertJSONSchemaToJsonSchemaProperty(schema.items[0])
          : { type: 'string' };
    } else {
      result.items = convertJSONSchemaToJsonSchemaProperty(schema.items);
    }
  }

  return result;
}

// Helper function to convert JSON schema types to Gemini types
function convertPropertiesToGeminiTypes(
  properties: Record<string, JsonSchemaProperty>,
): Record<
  string,
  { type: Type; description?: string; items?: { type: Type } }
> {
  if (!properties || typeof properties !== 'object') {
    return {};
  }

  const convertedProperties: Record<
    string,
    { type: Type; description?: string; items?: { type: Type } }
  > = {};

  for (const [key, value] of Object.entries(properties)) {
    const propType = value.type;

    switch (propType) {
      case 'string':
        convertedProperties[key] = { type: Type.STRING };
        break;
      case 'number':
      case 'integer':
        convertedProperties[key] = { type: Type.NUMBER };
        break;
      case 'boolean':
        convertedProperties[key] = { type: Type.BOOLEAN };
        break;
      case 'array':
        convertedProperties[key] = {
          type: Type.ARRAY,
          items: value.items
            ? convertSinglePropertyToGeminiType(value.items)
            : { type: Type.STRING },
        };
        break;
      case 'object':
        convertedProperties[key] = { type: Type.OBJECT };
        break;
      default:
        convertedProperties[key] = { type: Type.STRING };
        break;
    }

    if (value.description && typeof value.description === 'string') {
      convertedProperties[key].description = value.description;
    }
  }

  return convertedProperties;
}

// Helper function to sanitize JSON schema for Cerebras (remove unsupported fields)
function sanitizeSchemaForCerebras(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  if (!schema || typeof schema !== 'object') {
    return schema;
  }

  const sanitized = { ...schema };

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

// Helper function to convert a single property type
function convertSinglePropertyToGeminiType(prop: JsonSchemaProperty): {
  type: Type;
  items?: { type: Type };
} {
  switch (prop.type) {
    case 'string':
      return { type: Type.STRING };
    case 'number':
    case 'integer':
      return { type: Type.NUMBER };
    case 'boolean':
      return { type: Type.BOOLEAN };
    case 'array':
      return { type: Type.ARRAY };
    case 'object':
      return { type: Type.OBJECT };
    default:
      return { type: Type.STRING };
  }
}

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

// Updated tool conversion for Gemini - use parameters with Type enums
function convertMCPToolToProviderFormat(
  mcpTool: MCPTool,
  provider: AIServiceProvider,
): ProviderToolType {
  validateTool(mcpTool);

  // Extract properties and required fields from the structured schema
  const properties = mcpTool.inputSchema.properties || {};
  const required = mcpTool.inputSchema.required || [];

  const commonParameters = {
    type: 'object' as const,
    properties: properties,
    required: required,
  };

  switch (provider) {
    case AIServiceProvider.OpenAI:
    case AIServiceProvider.Fireworks:
      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: commonParameters,
        },
      } satisfies OpenAIChatCompletionTool;
    case AIServiceProvider.Groq:
      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: commonParameters,
        },
      };
    case AIServiceProvider.Anthropic:
      return {
        name: mcpTool.name,
        description: mcpTool.description,
        input_schema: commonParameters,
      };
    case AIServiceProvider.Gemini:
      // Use parameters with Type enums for Google GenAI SDK
      return {
        name: mcpTool.name,
        description: mcpTool.description,
        parameters: {
          type: Type.OBJECT,
          properties: convertPropertiesToGeminiTypes(
            Object.fromEntries(
              Object.entries(mcpTool.inputSchema.properties || {}).map(
                ([key, value]) => [
                  key,
                  convertJSONSchemaToJsonSchemaProperty(value),
                ],
              ),
            ),
          ),
          required: mcpTool.inputSchema.required || [],
        },
      };
    case AIServiceProvider.Cerebras: {
      // Cerebras doesn't support certain JSON schema fields like 'minimum', 'maximum', etc.
      const sanitizedParameters = sanitizeSchemaForCerebras(commonParameters);
      logger.info('Cerebras tool conversion:', {
        original: commonParameters,
        sanitized: sanitizedParameters,
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
    case AIServiceProvider.Ollama:
      return {
        type: 'function',
        function: {
          name: mcpTool.name,
          description: mcpTool.description,
          parameters: commonParameters,
        },
      } satisfies OllamaTool;
    case AIServiceProvider.Empty:
      throw new AIServiceError(
        `Tool conversion not supported for Empty AIServiceProvider`,
        AIServiceProvider.Empty,
      );
  }
}

export function convertMCPToolsToProviderTools(
  mcpTools: MCPTool[],
  provider: AIServiceProvider,
): ProviderToolsType {
  if (provider === AIServiceProvider.Gemini) {
    return mcpTools.map(
      (tool) =>
        convertMCPToolToProviderFormat(tool, provider) as FunctionDeclaration,
    );
  }
  return mcpTools.map((tool) => convertMCPToolToProviderFormat(tool, provider));
}

// Cerebras-specific tool conversion for type safety
export function convertMCPToolsToCerebrasTools(
  mcpTools: MCPTool[],
): Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool[] {
  return mcpTools.map((tool) => {
    validateTool(tool);

    const properties = tool.inputSchema.properties || {};
    const required = tool.inputSchema.required || [];

    const commonParameters = {
      type: 'object' as const,
      properties: properties,
      required: required,
    };

    // Cerebras doesn't support certain JSON schema fields like 'minimum', 'maximum', etc.
    const sanitizedParameters = sanitizeSchemaForCerebras(commonParameters);

    return {
      type: 'function',
      function: {
        name: tool.name,
        description: tool.description,
        parameters: sanitizedParameters,
      },
    } satisfies Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool;
  });
}
