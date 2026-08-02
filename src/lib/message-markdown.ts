import type { AttachmentReference, Message, ToolCall } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';

const DEFAULT_MAX_BYTES = 1024 * 1024;
const EXCLUDED_SOURCES = new Set([
  'compact-summary',
  'compaction-instruction',
  'recovery',
]);

export interface MessagesToMarkdownOptions {
  maxMessages?: number;
  maxBytes?: number;
  includeThinking?: boolean;
  includeToolCalls?: boolean;
  includeTimestamps?: boolean;
  includeSystem?: boolean;
}

export interface MessagesToMarkdownResult {
  content: string;
  truncated: boolean;
  omittedCount?: number;
}

type ResolvedMarkdownOptions = Required<
  Pick<
    MessagesToMarkdownOptions,
    | 'includeThinking'
    | 'includeToolCalls'
    | 'includeTimestamps'
    | 'includeSystem'
  >
> & {
  maxBytes: number;
  maxMessages?: number;
};

function shouldIncludeMessage(
  message: Message,
  options: Pick<ResolvedMarkdownOptions, 'includeSystem'>,
): boolean {
  if (message.isStreaming) {
    return false;
  }
  if (!options.includeSystem && message.role === 'system') {
    return false;
  }
  if (message.source && EXCLUDED_SOURCES.has(message.source)) {
    return false;
  }
  return true;
}

function formatRoleHeader(role: Message['role']): string {
  switch (role) {
    case 'user':
      return 'User';
    case 'assistant':
      return 'Assistant';
    case 'tool':
      return 'Tool';
    case 'system':
      return 'System';
    default:
      return role;
  }
}

function formatToolArguments(argumentsJson: string): string {
  try {
    return JSON.stringify(JSON.parse(argumentsJson), null, 2);
  } catch {
    return argumentsJson;
  }
}

function formatToolCalls(toolCalls: ToolCall[]): string {
  return toolCalls
    .map((toolCall) => {
      const args = formatToolArguments(toolCall.function.arguments);
      return `**Tool:** ${toolCall.function.name}\n\`\`\`json\n${args}\n\`\`\``;
    })
    .join('\n\n');
}

function formatContentItem(
  item: MCPContent,
  options: Pick<
    ResolvedMarkdownOptions,
    'includeThinking' | 'includeToolCalls'
  >,
): string | null {
  switch (item.type) {
    case 'text':
      return item.text;
    case 'thinking':
      if (!options.includeThinking) {
        return null;
      }
      return `<details>\n<summary>Thinking</summary>\n\n${item.thinking}\n</details>`;
    case 'tool_call':
      if (!options.includeToolCalls) {
        return null;
      }
      return formatToolCalls([
        {
          id: item.id,
          type: 'function',
          function: {
            name: item.name,
            arguments: item.arguments,
          },
        },
      ]);
    case 'image':
      return `[Image: ${item.mimeType}]`;
    case 'audio':
      return `[Audio: ${item.mimeType}]`;
    case 'video':
      return `[Video: ${item.mimeType}]`;
    case 'resource_link':
      return `[Resource Link: ${item.name}](${item.uri})`;
    case 'resource': {
      const resource = item.resource;
      const mimeType = resource?.mimeType ?? 'unknown';
      const uri = resource?.uri;
      return uri
        ? `[UI Resource: ${mimeType} - ${uri}]`
        : `[UI Resource: ${mimeType}]`;
    }
  }
}

function formatAttachments(attachments: AttachmentReference[]): string {
  return attachments
    .map((attachment) => {
      const lines = [
        `**Attachment:** ${attachment.filename} (${attachment.mimeType}, ${attachment.size} bytes)`,
      ];
      if (attachment.preview.trim()) {
        const preview = attachment.preview
          .split('\n')
          .map((line) => `> ${line}`)
          .join('\n');
        lines.push(preview);
      }
      return lines.join('\n');
    })
    .join('\n\n');
}

function formatTimestamp(createdAt: Date): string {
  return `*${createdAt.toISOString()}*`;
}

function resolveMarkdownOptions(
  options: MessagesToMarkdownOptions = {},
): ResolvedMarkdownOptions {
  return {
    maxMessages: options.maxMessages,
    maxBytes: options.maxBytes ?? DEFAULT_MAX_BYTES,
    includeThinking: options.includeThinking ?? false,
    includeToolCalls: options.includeToolCalls ?? true,
    includeTimestamps: options.includeTimestamps ?? false,
    includeSystem: options.includeSystem ?? false,
  };
}

function formatSingleMessage(
  message: Message,
  options: ResolvedMarkdownOptions,
): string {
  const parts: string[] = [`## ${formatRoleHeader(message.role)}`];

  if (options.includeTimestamps && message.createdAt) {
    parts.push(formatTimestamp(message.createdAt));
  }

  if (options.includeThinking && message.thinking) {
    parts.push(
      `<details>\n<summary>Thinking</summary>\n\n${message.thinking}\n</details>`,
    );
  }

  const bodyParts: string[] = [];
  for (const item of message.content ?? []) {
    const formatted = formatContentItem(item, options);
    if (formatted) {
      bodyParts.push(formatted);
    }
  }

  if (options.includeToolCalls && message.tool_calls?.length) {
    bodyParts.push(formatToolCalls(message.tool_calls));
  }

  if (message.tool_call_id) {
    bodyParts.push(`*Tool Call ID: ${message.tool_call_id}*`);
  }

  if (message.error?.displayMessage) {
    bodyParts.push(`**Error:** ${message.error.displayMessage}`);
  }

  if (message.attachments?.length) {
    bodyParts.push(formatAttachments(message.attachments));
  }

  const body = bodyParts.join('\n\n').trim();
  if (body) {
    parts.push(body);
  }

  return parts.join('\n\n');
}

/**
 * Format a single message as Markdown.
 * Unlike {@link messagesToMarkdown}, this does not skip streaming messages —
 * useful for per-bubble clipboard/export actions.
 */
export function messageToMarkdown(
  message: Message,
  options: MessagesToMarkdownOptions = {},
): string {
  return formatSingleMessage(message, resolveMarkdownOptions(options));
}

export function messagesToMarkdown(
  messages: Message[],
  options: MessagesToMarkdownOptions = {},
): MessagesToMarkdownResult {
  const resolvedOptions = resolveMarkdownOptions(options);

  let eligible = messages.filter((message) =>
    shouldIncludeMessage(message, resolvedOptions),
  );

  let omittedCount = 0;
  if (
    resolvedOptions.maxMessages !== undefined &&
    eligible.length > resolvedOptions.maxMessages
  ) {
    omittedCount = eligible.length - resolvedOptions.maxMessages;
    eligible = eligible.slice(-resolvedOptions.maxMessages);
  }

  const encoder = new TextEncoder();
  const sections: string[] = [];
  let totalBytes = 0;

  for (let index = 0; index < eligible.length; index += 1) {
    const section = formatSingleMessage(eligible[index], resolvedOptions);
    const sectionBytes = encoder.encode(section).length;
    const separatorBytes =
      sections.length > 0 ? encoder.encode('\n\n---\n\n').length : 0;

    if (totalBytes + separatorBytes + sectionBytes > resolvedOptions.maxBytes) {
      omittedCount += eligible.length - index;
      break;
    }

    if (sections.length > 0) {
      sections.push('---');
    }
    sections.push(section);
    totalBytes += separatorBytes + sectionBytes;
  }

  const truncated = omittedCount > 0;
  let content = sections.join('\n\n');

  if (truncated) {
    const notice = `> *Note: ${omittedCount} message(s) omitted due to size limits.*`;
    content = content ? `${notice}\n\n${content}` : notice;
  }

  return {
    content,
    truncated,
    omittedCount: truncated ? omittedCount : undefined,
  };
}
