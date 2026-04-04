import type { MCPTool } from '@/lib/mcp';

export function stableStringify(value: unknown): string {
  if (value === null || typeof value !== 'object') {
    if (typeof value === 'bigint') {
      return `{"$bigint":${JSON.stringify(value.toString())}}`;
    }

    const serialized = JSON.stringify(value);
    if (serialized !== undefined) {
      return serialized;
    }

    return JSON.stringify(String(value));
  }

  if (Array.isArray(value)) {
    return `[${value.map((item) => stableStringify(item)).join(',')}]`;
  }

  const entries = Object.entries(value as Record<string, unknown>).sort(
    ([leftKey], [rightKey]) => leftKey.localeCompare(rightKey),
  );

  return `{${entries
    .map(
      ([key, nestedValue]) =>
        `${JSON.stringify(key)}:${stableStringify(nestedValue)}`,
    )
    .join(',')}}`;
}

export function stableHashKeyPart(value: string): string {
  let hash = 2166136261;

  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }

  return (hash >>> 0).toString(16);
}

function stableClone<T>(value: T): T {
  if (value === null || typeof value !== 'object') {
    return value;
  }

  if (Array.isArray(value)) {
    return value.map((item) => stableClone(item)) as T;
  }

  const sortedEntries = Object.entries(value as Record<string, unknown>)
    .sort(([leftKey], [rightKey]) => leftKey.localeCompare(rightKey))
    .map(([key, nestedValue]) => [key, stableClone(nestedValue)]);

  return Object.fromEntries(sortedEntries) as T;
}

export function normalizeAvailableTools(tools: MCPTool[]): MCPTool[] {
  return tools
    .map((tool) => ({
      ...tool,
      inputSchema: stableClone(tool.inputSchema),
      outputSchema: tool.outputSchema
        ? stableClone(tool.outputSchema)
        : undefined,
      annotations: tool.annotations ? stableClone(tool.annotations) : undefined,
    }))
    .sort((left, right) => {
      const leftSignature = [
        left.name,
        left.title ?? '',
        left.description,
        stableStringify(left.inputSchema),
        stableStringify(left.outputSchema),
        stableStringify(left.annotations),
        left.backend ?? '',
      ].join('\n');

      const rightSignature = [
        right.name,
        right.title ?? '',
        right.description,
        stableStringify(right.inputSchema),
        stableStringify(right.outputSchema),
        stableStringify(right.annotations),
        right.backend ?? '',
      ].join('\n');

      return leftSignature.localeCompare(rightSignature);
    });
}
