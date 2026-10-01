import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import { getToolStructuredContent } from '@/lib/tool-call-utils';
import { parseReportResult } from '@/features/agent/components/tool-structured/types';
import { composeReportResultMarkdown } from '@/features/agent/lib/markdown-document-export';

export type MessageDocumentExportKind = 'markdown' | 'html';

export interface ResolvedMessageDocument {
  /** Source body for copy / export (markdown or HTML fragment). */
  content: string;
  fileBaseName: string;
  exportKind: MessageDocumentExportKind;
  source: 'reportResult' | 'presentInteractive';
}

type ResourceLike = {
  type: 'resource';
  resource?: {
    uri?: string;
    mimeType?: string;
    text?: string;
  };
  serviceInfo?: { toolName?: string };
};

function asResourceItem(item: MCPContent): ResourceLike | null {
  if (!item || typeof item !== 'object') {
    return null;
  }
  if ((item as { type?: string }).type !== 'resource') {
    return null;
  }
  return item as ResourceLike;
}

function tryReportResultDocument(
  structured: unknown,
): ResolvedMessageDocument | null {
  const parsed = parseReportResult(structured);
  if (!parsed) {
    return null;
  }
  return {
    content: composeReportResultMarkdown({
      title: parsed.title,
      criteria: parsed.criteria,
      proof: parsed.proof,
      result: parsed.result,
    }),
    fileBaseName: parsed.title?.trim() || 'result',
    exportKind: 'markdown',
    source: 'reportResult',
  };
}

/**
 * Pull presentInteractive source from the embedded `#raw-data` JSON blob.
 */
export function extractPresentInteractiveRawData(
  html: string,
): string | null {
  const match = html.match(
    /<script[^>]*\bid=["']raw-data["'][^>]*>([\s\S]*?)<\/script>/i,
  );
  if (!match?.[1]) {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(match[1].trim());
    return typeof parsed === 'string' ? parsed : null;
  } catch {
    return null;
  }
}

function extractPresentInteractiveTitle(html: string): string | null {
  const match = html.match(
    /<div[^>]*\bid=["']content-title["'][^>]*>([\s\S]*?)<\/div>/i,
  );
  if (!match?.[1] || /hidden/i.test(match[0])) {
    return null;
  }
  const title = match[1].replace(/<[^>]+>/g, '').trim();
  return title || null;
}

function tryPresentInteractiveDocument(
  content: MCPContent[],
): ResolvedMessageDocument | null {
  for (const item of content) {
    const resourceItem = asResourceItem(item);
    if (!resourceItem?.resource?.text) {
      continue;
    }
    const html = resourceItem.resource.text;
    const uri = resourceItem.resource.uri ?? '';
    const toolName = resourceItem.serviceInfo?.toolName ?? '';
    const isInteractive =
      uri.startsWith('ui://interactive/') ||
      toolName === 'presentInteractive' ||
      html.includes("id='raw-data'") ||
      html.includes('id="raw-data"');
    if (!isInteractive) {
      continue;
    }
    const raw = extractPresentInteractiveRawData(html);
    if (!raw?.trim()) {
      continue;
    }
    const isMarkdown =
      html.includes("id='md-root'") || html.includes('id="md-root"');
    return {
      content: raw.endsWith('\n') ? raw : `${raw}\n`,
      fileBaseName:
        extractPresentInteractiveTitle(html) || 'present-interactive',
      exportKind: isMarkdown ? 'markdown' : 'html',
      source: 'presentInteractive',
    };
  }
  return null;
}

/**
 * Tool-call ids visible on this bubble: message.tool_calls plus interleaved
 * displayContent tool_call items (full tool-group transcript).
 */
function collectBubbleToolCallIds(
  message: Message,
  displayContent?: MCPContent[],
): string[] {
  const ids: string[] = [];
  const seen = new Set<string>();

  const push = (id: string) => {
    if (!id || seen.has(id)) {
      return;
    }
    seen.add(id);
    ids.push(id);
  };

  for (const toolCall of message.tool_calls ?? []) {
    push(toolCall.id);
  }

  const content = displayContent ?? message.content ?? [];
  if (!Array.isArray(content)) {
    return ids;
  }
  for (const item of content) {
    if (item.type === 'tool_call') {
      push(item.id);
    }
  }

  return ids;
}

/**
 * Resolve a tool result for a call id, including `_dupN` keys from
 * useMessageGrouping when the model reuses call ids.
 */
function* iterToolResultsForCallId(
  toolResultsMap: Map<string, Message>,
  toolCallId: string,
): Generator<Message> {
  const primary = toolResultsMap.get(toolCallId);
  if (primary) {
    yield primary;
  }

  let seq = 1;
  while (true) {
    const dup = toolResultsMap.get(`${toolCallId}_dup${seq}`);
    if (!dup) {
      break;
    }
    yield dup;
    seq += 1;
  }
}

function tryDocumentFromToolResult(
  toolResult: Message,
): ResolvedMessageDocument | null {
  return (
    tryReportResultDocument(getToolStructuredContent(toolResult)) ??
    tryPresentInteractiveDocument(toolResult.content ?? [])
  );
}

/**
 * Prefer UI tool document bodies (reportResult / presentInteractive) over
 * plain message text so MessageActionBar copy/export matches what the user sees.
 *
 * Boundary: only this bubble's tool_calls / displayContent ids — never other
 * bubbles in the session. Among matches in the bubble, the newest wins.
 */
export function resolveMessageDocument(
  message: Message,
  options: {
    displayContent?: MCPContent[];
    toolResultsMap?: Map<string, Message>;
  } = {},
): ResolvedMessageDocument | null {
  const fromSelf = tryReportResultDocument(
    getToolStructuredContent(message),
  );
  if (fromSelf) {
    return fromSelf;
  }

  if (options.toolResultsMap) {
    const toolCallIds = collectBubbleToolCallIds(
      message,
      options.displayContent,
    );
    let latestInBubble: ResolvedMessageDocument | null = null;

    for (const toolCallId of toolCallIds) {
      for (const toolResult of iterToolResultsForCallId(
        options.toolResultsMap,
        toolCallId,
      )) {
        const fromResult = tryDocumentFromToolResult(toolResult);
        if (fromResult) {
          // Keep the last match so a later reportResult wins over earlier UI docs.
          latestInBubble = fromResult;
        }
      }
    }

    if (latestInBubble) {
      return latestInBubble;
    }
  }

  const content = options.displayContent ?? message.content ?? [];
  return tryPresentInteractiveDocument(content);
}
