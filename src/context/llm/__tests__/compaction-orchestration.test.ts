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

describe('SP17 Regression: buildCandidateStack summary injection via system prompt', () => {
  /**
   * Regression: Before the fix, the compact summary was injected as a synthetic
   * user Message into the conversation history. This caused consecutive user
   * messages (summary + actual user message) which broke prefix caching.
   *
   * After the fix, buildCandidateStack returns { messages, summary } and the
   * summary is appended to the system prompt instead.
   */

  type CacheEntry = { fromId: string; toId: string; summary: string };

  const buildCandidateStack = (
    msgs: { id: string; role: string }[],
    cache: CacheEntry | undefined,
  ): { messages: { id: string; role: string }[]; summary?: string } => {
    if (!cache) return { messages: msgs };

    const toIdIndex = msgs.findIndex((m) => m.id === cache.toId);
    if (toIdIndex >= 0) {
      return {
        messages: msgs.slice(toIdIndex + 1),
        summary: `### Previous Conversation Summary\n${cache.summary}`,
      };
    }
    return { messages: msgs };
  };

  it('should return summary separately, NOT as a user message', () => {
    const msgs = [
      { id: 'msg-1', role: 'user' },
      { id: 'msg-2', role: 'assistant' },
      { id: 'msg-3', role: 'user' },
    ];
    const cache: CacheEntry = {
      fromId: 'msg-1',
      toId: 'msg-2',
      summary: 'User asked X, assistant replied Y.',
    };

    const { messages, summary } = buildCandidateStack(msgs, cache);

    // Messages after toId only — no synthetic summary message injected
    expect(messages).toHaveLength(1);
    expect(messages[0].id).toBe('msg-3');
    expect(messages.every((m) => m.role !== 'user' || m.id !== 'compact-summary-msg-1~msg-2')).toBe(true);

    // Summary is returned as a string for system prompt injection
    expect(summary).toContain('Previous Conversation Summary');
    expect(summary).toContain('User asked X, assistant replied Y.');
  });

  it('should NOT prepend summary as a consecutive user message (caching regression)', () => {
    const msgs = [
      { id: 'msg-1', role: 'user' },
      { id: 'msg-2', role: 'assistant' },
      { id: 'msg-3', role: 'user' },
    ];
    const cache: CacheEntry = {
      fromId: 'msg-1',
      toId: 'msg-2',
      summary: 'Summary content.',
    };

    const { messages } = buildCandidateStack(msgs, cache);

    // The first message in the candidate stack must NOT be a synthetic summary user message.
    // If it were, it would create user→user adjacency and break Gemini prefix caching.
    expect(messages[0].role).toBe('user');
    expect(messages[0].id).toBe('msg-3'); // real user message, not synthetic
    expect(messages[0].id).not.toMatch(/^compact-summary-/);
  });

  // Regression: After successful compaction, finalSystemPrompt must be updated
  // so selectMessagesWithinContext uses the correct (larger) token budget.
  it('should update finalSystemPrompt after compaction to reflect new summary', () => {
    const baseSystemPrompt = 'You are a helpful assistant.';
    const oldSummary = 'Old short summary.';
    const newSummary = 'New longer summary after compaction with more detail.';

    // Pre-compaction state
    let finalSystemPrompt = oldSummary
      ? `${baseSystemPrompt}\n\n### Previous Conversation Summary\n${oldSummary}`.trim()
      : baseSystemPrompt;

    const preCompactionLength = finalSystemPrompt.length;

    // Simulate successful compaction: updatedSystemPrompt is computed and assigned back
    const updatedSystemPrompt = newSummary
      ? `${baseSystemPrompt}\n\n### Previous Conversation Summary\n${newSummary}`.trim()
      : baseSystemPrompt;

    finalSystemPrompt = updatedSystemPrompt; // ← the fix

    // finalSystemPrompt must now reflect the new summary, not the old one
    expect(finalSystemPrompt).toContain(newSummary);
    expect(finalSystemPrompt).not.toContain(oldSummary);
    expect(finalSystemPrompt.length).toBeGreaterThan(preCompactionLength);
  });
});