interface IntegerInputOptions {
  fallback: number;
  min?: number;
  max?: number;
}

function clampNumber(
  value: number,
  { min, max }: Omit<IntegerInputOptions, 'fallback'>,
): number {
  if (typeof min === 'number' && value < min) {
    return min;
  }

  if (typeof max === 'number' && value > max) {
    return max;
  }

  return value;
}

export function parseIntegerInput(
  rawValue: string,
  options: IntegerInputOptions,
): number {
  const parsedValue = Number.parseInt(rawValue, 10);

  if (Number.isNaN(parsedValue)) {
    return options.fallback;
  }

  return clampNumber(parsedValue, options);
}

/**
 * Returns true when `rawValue` is a finite integer already inside [min, max].
 * Used by number inputs to avoid clamping mid-keystroke (e.g. typing "256" with min=32).
 */
export function isIntegerInRange(
  rawValue: string,
  { min, max }: Omit<IntegerInputOptions, 'fallback'>,
): boolean {
  if (rawValue.trim() === '') {
    return false;
  }

  const parsedValue = Number.parseInt(rawValue, 10);
  if (Number.isNaN(parsedValue)) {
    return false;
  }

  // Reject partial numeric strings that parseInt would accept ("12abc" → 12).
  if (String(parsedValue) !== rawValue.trim()) {
    return false;
  }

  if (typeof min === 'number' && parsedValue < min) {
    return false;
  }

  if (typeof max === 'number' && parsedValue > max) {
    return false;
  }

  return true;
}

interface FloatInputOptions {
  fallback: number;
  min?: number;
  max?: number;
}

export function parseFloatInput(
  rawValue: string,
  options: FloatInputOptions,
): number {
  const parsedValue = Number.parseFloat(rawValue);

  if (Number.isNaN(parsedValue)) {
    return options.fallback;
  }

  return clampNumber(parsedValue, options);
}

/**
 * Returns true when `rawValue` is a finite number already inside [min, max].
 * Allows decimal values mid-edit when they are complete and in range.
 */
export function isFloatInRange(
  rawValue: string,
  { min, max }: Omit<FloatInputOptions, 'fallback'>,
): boolean {
  const trimmed = rawValue.trim();
  if (trimmed === '') {
    return false;
  }

  // Require a complete numeric literal (optional leading minus, optional fraction).
  if (!/^-?\d+(\.\d+)?$/.test(trimmed)) {
    return false;
  }

  const parsedValue = Number.parseFloat(trimmed);
  if (Number.isNaN(parsedValue) || !Number.isFinite(parsedValue)) {
    return false;
  }

  if (typeof min === 'number' && parsedValue < min) {
    return false;
  }

  if (typeof max === 'number' && parsedValue > max) {
    return false;
  }

  return true;
}

interface KilobyteInputOptions {
  fallbackKilobytes: number;
  minKilobytes: number;
  maxKilobytes: number;
}

export function parseKilobytesInputToBytes(
  rawValue: string,
  options: KilobyteInputOptions,
): number {
  const kilobytes = parseIntegerInput(rawValue, {
    fallback: options.fallbackKilobytes,
    min: options.minKilobytes,
    max: options.maxKilobytes,
  });

  return kilobytes * 1024;
}

export function formatBytesAsKilobytes(
  valueInBytes: number | undefined,
  fallbackBytes: number,
): number {
  return (valueInBytes ?? fallbackBytes) / 1024;
}
