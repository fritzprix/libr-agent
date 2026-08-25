import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';

/**
 * Retry guidance for contaminated compaction output.
 * Must be appended to the compaction-instruction *tail* message — never to the
 * system prompt — so provider prompt-cache prefixes stay intact (issue #1797).
 */
export const COMPACTION_RETRY_NO_TOOL_NOTE =
  'RETRY INSTRUCTION: The previous compaction response contained tool-call markup or execution syntax. Output plain markdown summary text only. Do not call tools, emit tool-call markup, or include pseudo-tool XML/JSON.';

function isCompactionInstructionMessage(message: Message): boolean {
  if (message.source === 'compaction-instruction') {
    return true;
  }
  return message.id.startsWith('compaction-instruction-');
}

function appendRetryNoteToContent(content: MCPContent[]): MCPContent[] {
  let appended = false;
  const next = content.map((part) => {
    if (appended || part.type !== 'text' || typeof part.text !== 'string') {
      return part;
    }
    appended = true;
    if (part.text.includes(COMPACTION_RETRY_NO_TOOL_NOTE)) {
      return part;
    }
    return {
      ...part,
      text: `${part.text}\n\n${COMPACTION_RETRY_NO_TOOL_NOTE}`,
    };
  });

  if (appended) {
    return next;
  }

  return [
    ...content,
    { type: 'text', text: COMPACTION_RETRY_NO_TOOL_NOTE } satisfies MCPContent,
  ];
}

/**
 * Keep system prompt + tool schemas identical to the parent turn; only extend the
 * synthetic compaction-instruction user message at the tail.
 */
export function applyCompactionRetryTailInstruction(
  messages: Message[],
): Message[] {
  if (messages.length === 0) {
    return messages;
  }

  let targetIdx = -1;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (isCompactionInstructionMessage(messages[i])) {
      targetIdx = i;
      break;
    }
  }
  if (targetIdx < 0) {
    targetIdx = messages.length - 1;
  }

  return messages.map((message, index) => {
    if (index !== targetIdx) {
      return message;
    }
    return {
      ...message,
      content: appendRetryNoteToContent(message.content),
    };
  });
}
