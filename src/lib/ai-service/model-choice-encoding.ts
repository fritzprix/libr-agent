const MODEL_CHOICE_SEPARATOR = ':::';

export interface DecodedModelChoice {
  provider: string;
  model: string;
}

/**
 * Serializes provider + model into a single Radix Select value.
 * Uses a triple-colon delimiter; model ids may contain single colons or slashes.
 */
export function encodeModelChoice(provider: string, model: string): string {
  return `${provider}${MODEL_CHOICE_SEPARATOR}${model}`;
}

/**
 * Parses a grouped select value back into provider and model.
 * Returns null when the value is empty or malformed.
 */
export function decodeModelChoice(
  value: string | undefined | null,
): DecodedModelChoice | null {
  if (!value) {
    return null;
  }

  const separatorIndex = value.indexOf(MODEL_CHOICE_SEPARATOR);
  if (separatorIndex <= 0) {
    return null;
  }

  const provider = value.slice(0, separatorIndex);
  const model = value.slice(separatorIndex + MODEL_CHOICE_SEPARATOR.length);

  if (!provider || !model) {
    return null;
  }

  return { provider, model };
}
