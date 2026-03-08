import { describe, it, expect } from 'vitest';
import { calculateCompactThreshold, calculateEffectiveContextLimit } from '../compact-utils';
import type { ModelInfo } from '../llm-config-manager';

describe('compact-utils', () => {
  describe('calculateEffectiveContextLimit', () => {
    it('uses maxInputContext when it is smaller than safe input limit', () => {
      const modelInfo = { contextWindow: 128000 } as ModelInfo;
      const maxOutputTokens = 8192;
      const maxInputContext = 32768;

      const { effectiveLimit, modelMaxLimit } = calculateEffectiveContextLimit(
        modelInfo,
        maxOutputTokens,
        maxInputContext,
      );
      
      expect(modelMaxLimit).toBe(128000);
      expect(effectiveLimit).toBe(32768);
    });

    it('uses safe input limit when maxInputContext is larger than available space', () => {
      const modelInfo = { contextWindow: 64000 } as ModelInfo;
      const maxOutputTokens = 8192;
      const maxInputContext = 128000;

      const { effectiveLimit, modelMaxLimit } = calculateEffectiveContextLimit(
        modelInfo,
        maxOutputTokens,
        maxInputContext,
      );
      
      expect(modelMaxLimit).toBe(64000);
      // safe input limit = 64000 - 8192 - 100 = 55708
      expect(effectiveLimit).toBe(55708);
    });

    it('falls back to default context window if model lacks contextWindow', () => {
      const modelInfo = {} as ModelInfo;
      const maxOutputTokens = 8192;
      const maxInputContext = 32768;

      const { effectiveLimit, modelMaxLimit } = calculateEffectiveContextLimit(
        modelInfo,
        maxOutputTokens,
        maxInputContext,
      );
      
      expect(modelMaxLimit).toBe(65536); // 64 * 1024
      expect(effectiveLimit).toBe(32768);
    });

    it('falls back to safe limit if no maxInputContext is provided', () => {
      const modelInfo = { contextWindow: 128000 } as ModelInfo;
      const maxOutputTokens = 8192;

      const { effectiveLimit, modelMaxLimit } = calculateEffectiveContextLimit(
        modelInfo,
        maxOutputTokens,
        undefined,
      );
      
      expect(modelMaxLimit).toBe(128000);
      expect(effectiveLimit).toBe(119708); // 128000 - 8192 - 100
    });
  });

  describe('calculateCompactThreshold', () => {
    it('returns 90% of the effective limit', () => {
      expect(calculateCompactThreshold(10000)).toBe(9000);
      expect(calculateCompactThreshold(32768)).toBe(29491); // Math.floor(32768 * 0.9)
    });
  });
});
