import type { TokenUsage } from './types';

const streamToolCallStartBrand: unique symbol = Symbol('streamToolCallStart');
const streamDirectToolCallBrand: unique symbol = Symbol('streamDirectToolCall');
const streamToolCallArgumentDeltaBrand: unique symbol = Symbol(
  'streamToolCallArgumentDelta',
);

export interface StreamToolCallFunctionUpdate {
  name?: string;
  arguments: string;
}

export interface StreamIndexedToolCallDelta {
  index: number;
  id?: string;
  type?: 'function';
  function: StreamToolCallFunctionUpdate;
}

export interface StreamDirectToolCall {
  index?: never;
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

export type StreamToolCallUpdate =
  | StreamIndexedToolCallDelta
  | StreamDirectToolCall;

export interface StreamToolCallStart {
  index: number;
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

export interface ParsedStreamChunk {
  content?: string;
  thinking?: string;
  thinkingSignature?: string;
  usage?: TokenUsage;
  tool_call_starts?: StreamToolCallStart[];
  tool_calls?: StreamToolCallUpdate[];
}

export type SerializableStreamToolCallStart = StreamToolCallStart & {
  readonly [streamToolCallStartBrand]: true;
};

export type SerializableStreamDirectToolCall = StreamDirectToolCall & {
  readonly [streamDirectToolCallBrand]: true;
};

export type SerializableStreamToolCallArgumentDelta =
  StreamIndexedToolCallDelta & {
    readonly [streamToolCallArgumentDeltaBrand]: true;
  };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isTokenUsage(value: unknown): value is TokenUsage {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value.promptTokens === 'number' &&
    typeof value.completionTokens === 'number' &&
    typeof value.totalTokens === 'number'
  );
}

function isToolCallFunctionUpdate(
  value: unknown,
): value is StreamToolCallFunctionUpdate {
  if (!isRecord(value) || typeof value.arguments !== 'string') {
    return false;
  }

  return value.name === undefined || typeof value.name === 'string';
}

function isToolCallType(value: unknown): value is 'function' {
  return value === undefined || value === 'function';
}

function isIndexedToolCallDelta(
  value: unknown,
): value is StreamIndexedToolCallDelta {
  if (!isRecord(value) || typeof value.index !== 'number') {
    return false;
  }

  if (!isToolCallType(value.type)) {
    return false;
  }

  if (value.id !== undefined && typeof value.id !== 'string') {
    return false;
  }

  return isToolCallFunctionUpdate(value.function);
}

function isDirectToolCall(value: unknown): value is StreamDirectToolCall {
  if (!isRecord(value) || 'index' in value || value.type !== 'function') {
    return false;
  }

  if (typeof value.id !== 'string') {
    return false;
  }

  if (!isRecord(value.function)) {
    return false;
  }

  return (
    typeof value.function.name === 'string' &&
    typeof value.function.arguments === 'string'
  );
}

function isToolCallUpdate(value: unknown): value is StreamToolCallUpdate {
  return isIndexedToolCallDelta(value) || isDirectToolCall(value);
}

function isToolCallStart(value: unknown): value is StreamToolCallStart {
  return (
    isIndexedToolCallDelta(value) &&
    typeof value.index === 'number' &&
    typeof value.id === 'string' &&
    typeof value.function.name === 'string'
  );
}

function getMetadataThinkingSignature(metadata: unknown): string | undefined {
  if (!isRecord(metadata)) {
    return undefined;
  }

  return typeof metadata.thinking_signature === 'string'
    ? metadata.thinking_signature
    : undefined;
}

export function createSerializableToolCallStart(
  index: number,
  id: string,
  name: string,
): SerializableStreamToolCallStart {
  return {
    index,
    id,
    type: 'function',
    function: {
      name,
      arguments: '',
    },
    [streamToolCallStartBrand]: true,
  };
}

export function createSerializableDirectToolCall(
  id: string,
  name: string,
  argumentsText: string,
): SerializableStreamDirectToolCall {
  return {
    id,
    type: 'function',
    function: {
      name,
      arguments: argumentsText,
    },
    [streamDirectToolCallBrand]: true,
  };
}

export function createSerializableToolCallArgumentDelta(
  index: number,
  argumentsText: string,
  options: {
    id?: string;
    name?: string;
  } = {},
): SerializableStreamToolCallArgumentDelta {
  return {
    index,
    ...(options.id ? { id: options.id } : {}),
    ...(options.id || options.name ? { type: 'function' as const } : {}),
    function: {
      ...(options.name ? { name: options.name } : {}),
      arguments: argumentsText,
    },
    [streamToolCallArgumentDeltaBrand]: true,
  };
}

export function serializeStreamContent(content: string): string {
  return JSON.stringify({ content } satisfies ParsedStreamChunk);
}

export function serializeStreamThinking(thinking: string): string {
  return JSON.stringify({ thinking } satisfies ParsedStreamChunk);
}

export function serializeThinkingSignature(thinkingSignature: string): string {
  return JSON.stringify({ thinkingSignature } satisfies ParsedStreamChunk);
}

export function serializeStreamUsage(usage: TokenUsage): string {
  return JSON.stringify({ usage } satisfies ParsedStreamChunk);
}

export function serializeToolCallStarts(
  toolCallStarts: SerializableStreamToolCallStart[],
): string {
  return JSON.stringify({
    tool_call_starts: toolCallStarts,
  } satisfies ParsedStreamChunk);
}

export function serializeToolCallArgumentDeltas(
  toolCalls: SerializableStreamToolCallArgumentDelta[],
): string {
  return JSON.stringify({ tool_calls: toolCalls } satisfies ParsedStreamChunk);
}

export function serializeDirectToolCalls(
  toolCalls: SerializableStreamDirectToolCall[],
): string {
  return JSON.stringify({ tool_calls: toolCalls } satisfies ParsedStreamChunk);
}

export function parseStreamChunk(rawChunk: string): ParsedStreamChunk {
  let parsed: unknown;

  try {
    parsed = JSON.parse(rawChunk);
  } catch {
    return { content: rawChunk };
  }

  if (typeof parsed === 'string') {
    return { content: parsed };
  }

  if (!isRecord(parsed)) {
    return {};
  }

  const chunk: ParsedStreamChunk = {};

  if (typeof parsed.content === 'string') {
    chunk.content = parsed.content;
  }

  if (typeof parsed.thinking === 'string') {
    chunk.thinking = parsed.thinking;
  }

  if (typeof parsed.thinkingSignature === 'string') {
    chunk.thinkingSignature = parsed.thinkingSignature;
  } else {
    const metadataSignature = getMetadataThinkingSignature(parsed.metadata);
    if (metadataSignature) {
      chunk.thinkingSignature = metadataSignature;
    }
  }

  if (isTokenUsage(parsed.usage)) {
    chunk.usage = parsed.usage;
  }

  if (Array.isArray(parsed.tool_call_starts)) {
    const starts = parsed.tool_call_starts.filter(isToolCallStart);
    if (starts.length > 0) {
      chunk.tool_call_starts = starts;
    }
  }

  if (Array.isArray(parsed.tool_calls)) {
    const updates = parsed.tool_calls.filter(isToolCallUpdate);
    if (updates.length > 0) {
      chunk.tool_calls = updates;
    }
  }

  return chunk;
}

export function isParsedIndexedToolCallDelta(
  value: unknown,
): value is StreamIndexedToolCallDelta {
  return isIndexedToolCallDelta(value);
}

export function isParsedDirectToolCall(
  value: unknown,
): value is StreamDirectToolCall {
  return isDirectToolCall(value);
}
