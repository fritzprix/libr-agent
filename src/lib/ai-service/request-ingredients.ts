export interface MessageIngredientSummary {
  messageCount: number;
  roleCounts: Record<string, number>;
  sourceCounts: Record<string, number>;
  compactSummaryCount: number;
  compactionInstructionCount: number;
  sessionContextCount: number;
  externalRequestCount: number;
  assistantToolCallCount: number;
}

export interface RequestIngredientMessageLike {
  role: string;
  source?: string | null;
  tool_calls?: ReadonlyArray<unknown> | null;
}

export function summarizeMessageIngredients(
  messages: ReadonlyArray<RequestIngredientMessageLike>,
): MessageIngredientSummary {
  const roleCounts: Record<string, number> = {};
  const sourceCounts: Record<string, number> = {};

  let compactSummaryCount = 0;
  let compactionInstructionCount = 0;
  let sessionContextCount = 0;
  let externalRequestCount = 0;
  let assistantToolCallCount = 0;

  for (const message of messages) {
    roleCounts[message.role] = (roleCounts[message.role] ?? 0) + 1;

    const source = String(message.source ?? 'none');
    sourceCounts[source] = (sourceCounts[source] ?? 0) + 1;

    if (source === 'compact-summary') {
      compactSummaryCount += 1;
    }
    if (source === 'compaction-instruction') {
      compactionInstructionCount += 1;
    }
    if (source === 'session-context') {
      sessionContextCount += 1;
    }
    if (
      source === 'ui' ||
      source === 'api' ||
      source === 'channel' ||
      source === 'scheduled_task'
    ) {
      externalRequestCount += 1;
    }
    if (message.tool_calls?.length) {
      assistantToolCallCount += 1;
    }
  }

  return {
    messageCount: messages.length,
    roleCounts,
    sourceCounts,
    compactSummaryCount,
    compactionInstructionCount,
    sessionContextCount,
    externalRequestCount,
    assistantToolCallCount,
  };
}
