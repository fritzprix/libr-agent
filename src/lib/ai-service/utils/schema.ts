export function ensureSchemaTypeField(
  schema: object | Record<string, unknown>,
): Record<string, unknown> {
  if (!schema || typeof schema !== 'object') {
    return { type: 'object', properties: {} };
  }

  const result = { ...(schema as Record<string, unknown>) };

  if (!result.type) {
    if (result.properties && typeof result.properties === 'object') {
      result.type = 'object';
    } else if (result.items) {
      result.type = 'array';
    } else {
      result.type = 'object';
    }
  }

  if (Array.isArray(result.type)) {
    const nonNullType = (result.type as string[]).find((t) => t !== 'null');
    result.type = nonNullType || 'string';
  }

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

  for (const key of ['oneOf', 'anyOf', 'allOf'] as const) {
    const value = result[key];
    if (Array.isArray(value)) {
      result[key] = value.map((item) =>
        typeof item === 'object' && item !== null
          ? ensureSchemaTypeField(item as Record<string, unknown>)
          : item,
      );
    }
  }

  return result;
}
