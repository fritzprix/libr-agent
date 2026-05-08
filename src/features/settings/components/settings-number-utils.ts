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
