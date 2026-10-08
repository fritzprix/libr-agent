import { describe, expect, it } from 'vitest';
import { normalizeComplexityModelMapping } from '../settings-service';

describe('normalizeComplexityModelMapping', () => {
  it('returns empty mapping for invalid input', () => {
    expect(normalizeComplexityModelMapping(null)).toEqual({});
    expect(normalizeComplexityModelMapping('x')).toEqual({});
    expect(normalizeComplexityModelMapping([])).toEqual({});
  });

  it('keeps valid level overrides and nulls invalid ones', () => {
    expect(
      normalizeComplexityModelMapping({
        low: { model: 'gpt-4o-mini', provider: 'openai' },
        normal: { model: '', provider: 'openai' },
        high: null,
      }),
    ).toEqual({
      low: { model: 'gpt-4o-mini', provider: 'openai' },
      normal: null,
      high: null,
    });
  });
});
