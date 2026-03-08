import { describe, it, expect } from 'vitest';
import { 
  calculateEffectiveContextLimit, 
  calculateCompactThreshold,
  findCompactionSplitIndex 
} from '../compact-utils';
import { calculateGroundedTotalTokens } from '../token-utils';
import type { Message } from '@/models/chat';
import type { ModelInfo } from '../llm-config-manager';

describe('SP17 Compliance: Message Context Management', () => {
  
  describe('1. Dynamic Max Context & Threshold (Spec #8, #2)', () => {
    it('should respect maxInputContext from settings if smaller than model limit', () => {
      const modelInfo = { contextWindow: 128000 } as ModelInfo;
      const { effectiveLimit } = calculateEffectiveContextLimit(modelInfo, 8192, 32000);
      expect(effectiveLimit).toBe(32000);
    });

    it('should calculate 90% threshold for compaction', () => {
      const threshold = calculateCompactThreshold(10000);
      expect(threshold).toBe(9000);
    });
  });

  describe('2. Grounded Token Estimation (Internal Improvement for Spec Accuracy)', () => {
    it('should use API billed tokens as baseline if available', () => {
      const messages: Message[] = [
        { id: '1', role: 'user', content: [{ type: 'text', text: 'hi' }] } as Message,
        { id: '2', role: 'assistant', content: [{ type: 'text', text: 'hello' }], usage: { promptTokens: 1000, completionTokens: 500, totalTokens: 1500 } } as Message,
        { id: '3', role: 'user', content: [{ type: 'text', text: 'new quest' }] } as Message,
      ];
      
      // 1500 (billed) + 100 (new user message estimate) = 1600
      const total = calculateGroundedTotalTokens(messages, 0, 0);
      // Note: In implementation, calculateGroundedTotalTokens uses internal estimateTokensBPE
      // We verify the logic flow here.
      expect(total).toBeGreaterThan(1500); 
    });

    it('should fallback to full BPE if summary is detected after last grounded point', () => {
      const messages: Message[] = [
        { id: '1', sessionId: 's', threadId: 's', role: 'assistant', content: [], usage: { totalTokens: 5000, promptTokens: 0, completionTokens: 0 } } as Message,
        { id: 'compact-summary-xxx', sessionId: 's', threadId: 's', role: 'user', content: [{ type: 'text', text: 'summary' }] } as Message,
        { id: '3', sessionId: 's', threadId: 's', role: 'user', content: [{ type: 'text', text: 'new' }] } as Message,
      ];
      // Should ignore 5000 and recalculate everything because summary changed the history
      const total = calculateGroundedTotalTokens(messages, 100, 50);
      // (BPE for summary ~3 tokens) + (BPE for 'new' ~3 tokens) + 100 + 50 = ~156
      expect(total).toBeLessThan(1000); 
    });
  });

  describe('3. Compaction Splitting Logic (Spec #2, #6)', () => {
    interface TestMsg { id: string; tokens: number }
    const messages: TestMsg[] = Array.from({ length: 20 }, (_, i) => ({ id: `${i}`, tokens: 100 }));
    const estimate = (m: TestMsg) => m.tokens;

    it('should keep roughly 50% of message budget for recent messages', () => {
      // Threshold 10000, System+Tools 2000 => Budget 8000. Keep 4000.
      // 20 messages of 100 tokens = 2000 total messages.
      // Since 2000 < 4000, it won't find a split point via budget.
      // It should fallback to halfway split because length > 10.
      const splitIdx = findCompactionSplitIndex(messages, estimate, 10000, 1000, 1000);
      expect(splitIdx).toBe(10); 
    });

    it('should split exactly at the point where recent messages exceed keepThreshold', () => {
      // 20 messages of 100 tokens.
      // Threshold 2000, System 0, Tools 0 => Budget 2000. Keep 1000.
      // Reverse loop: 19, 18, ... 10. (10 messages = 1000 tokens).
      // splitIdx should be 10 (keeping messages 10-19).
      const splitIdx = findCompactionSplitIndex(messages, estimate, 2000, 0, 0);
      expect(splitIdx).toBe(10);
    });

    it('should handle large system prompt correctly (Spec compliance for large workspaces)', () => {
      const spikeMessages: TestMsg[] = [
        { id: '1', tokens: 100 },
        { id: '2', tokens: 100 },
        { id: '3', tokens: 100 },
      ];
      // System prompt 30000, threshold 32000 => Budget 2000. Keep 1000.
      // Messages are only 300 tokens total. splitIdx will be 0.
      // Should NOT fallback because length < 10.
      const splitIdx = findCompactionSplitIndex(spikeMessages, estimate, 32000, 20000, 10000);
      expect(splitIdx).toBe(0);
    });
  });
});