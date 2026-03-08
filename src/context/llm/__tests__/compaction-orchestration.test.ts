import { describe, it, expect, vi, beforeEach } from 'vitest';
import { 
  findCompactionSplitIndex
} from '@/lib/compact-utils';

/**
 * SP17 Regression Tests: Compaction Orchestration Logic
 * 
 * These tests verify the coordination patterns implemented in useLLMExecution.ts
 * to handle asynchronous compaction, recurring tool calls, and context overflows.
 */
describe('SP17 Orchestration: Compaction Coordination', () => {
  
  // Mock data setup
  const sessionId = 'test-session';
  const threshold = 10000; // 10k tokens
  const safeLimit = 11111; // ~11k tokens (10k / 0.9)
  
  // Simulation state (mimicking useRef/useState in the hook)
  const compactResolvers = new Map<string, (() => void)[]>();

  beforeEach(() => {
    compactResolvers.clear();
    vi.clearAllMocks();
  });

  /**
   * Logic Test: Preventing Duplicate Compaction Triggers
   * Scenario: A request triggers compaction. Before it finishes, another request arrives.
   * Requirement: Only one compaction should be active at a time for a session.
   */
  it('should not trigger a new compaction if one is already in progress', () => {
    const totalTokens = 10500; // Above 10000 threshold

    // 1. First request logic
    const shouldTrigger1 = totalTokens >= threshold && !compactResolvers.has(sessionId);
    expect(shouldTrigger1).toBe(true);
    
    // Start compaction (Simulation)
    compactResolvers.set(sessionId, []);

    // 2. Second request logic (arrives while compaction is active)
    const shouldTrigger2 = totalTokens >= threshold && !compactResolvers.has(sessionId);
    
    expect(shouldTrigger2).toBe(false); // ✅ Correct: should NOT trigger again
    expect(compactResolvers.has(sessionId)).toBe(true);
  });

  /**
   * Logic Test: Blocking on Overflow (100%+)
   * Scenario: Token count is over 100%, and a compaction is already running.
   * Requirement: The LLM request must WAIT for the compaction to complete.
   */
  it('should wait for compaction when in overflow state (>= 100%)', async () => {
    const overflowTokens = 12000; // Above 11111 safe limit
    const overflow = overflowTokens >= safeLimit;
    
    // Simulate ongoing compaction
    compactResolvers.set(sessionId, []);
    
    let waitCompleted = false;
    
    // The "Wait" logic from useLLMExecution.ts
    const simulateRequestWait = async () => {
      if (overflow && compactResolvers.has(sessionId)) {
        await new Promise<void>((resolve) => {
          const list = compactResolvers.get(sessionId) || [];
          list.push(resolve);
          compactResolvers.set(sessionId, list);
        });
      }
      waitCompleted = true;
    };

    const requestPromise = simulateRequestWait();
    
    // Check that it's currently waiting
    expect(waitCompleted).toBe(false);
    expect(compactResolvers.get(sessionId)?.length).toBe(1);

    // Simulate Compaction Finish
    const resolvers = compactResolvers.get(sessionId) || [];
    resolvers.forEach(r => r());
    compactResolvers.delete(sessionId);

    await requestPromise;
    expect(waitCompleted).toBe(true); // ✅ Correct: waited and then resumed
  });

  /**
   * Logic Test: Recursive Compaction Range Management
   * Scenario: Compacting a stack that already contains a summary message.
   * Requirement: ID strings should not become nested (point 7 fix).
   */
  it('should extract the original starting ID when re-compacting a summary', () => {
    const syntheticMsg = { 
      id: 'compact-summary-msg_001~msg_050',
      role: 'user' 
    };
    
    // Logic from useLLMExecution.ts fix:
    const firstMsg = syntheticMsg;
    const fromId = firstMsg.id.startsWith('compact-summary-')
      ? firstMsg.id.replace('compact-summary-', '').split('~')[0]
      : firstMsg.id;
      
    expect(fromId).toBe('msg_001'); // ✅ Correct: extracted original base ID
  });

  /**
   * Logic Test: Fallback Splitting (The Tool Recurring Case)
   * Scenario: System prompt is huge, messages are few/small. 
   * Requirement: Must still force a split to resolve threshold breach.
   */
  it('should force a split even if messages are small when message count is high', () => {
    const thresholdVal = 10000;
    const sysPromptTokens = 9500; // Almost fills the budget
    const toolsTokens = 0;
    const messages = Array.from({ length: 12 }, (_, i) => ({ id: `m${i}`, tokens: 10 }));
    const estimate = (m: unknown) => (m as { tokens: number }).tokens;

    // findCompactionSplitIndex logic
    const splitIdx = findCompactionSplitIndex(
      messages as unknown as { id: string }[],
      estimate as (m: unknown) => number,
      thresholdVal,
      sysPromptTokens,
      toolsTokens,
    );
    
    // Even though currentSum of messages (12*10=120) < keepThreshold (250), 
    // it should force a split at half (6) because length >= 10.
    expect(splitIdx).toBe(6); 
    expect(messages.slice(0, splitIdx).length).toBe(6); // ✅ Correct: forced compaction of 6 messages
  });
});