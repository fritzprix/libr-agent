import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import {
  extractPresentInteractiveRawData,
  resolveMessageDocument,
} from '../message-document';
import {
  serializeMessageForClipboard,
  serializeMessageForDownload,
} from '../message-serialization';

function createMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'assistant',
    content: [],
    ...overrides,
  };
}

function presentInteractiveResource(html: string): MCPContent {
  return {
    type: 'resource',
    resource: {
      uri: 'ui://interactive/abc',
      mimeType: 'text/html',
      text: html,
    },
  } as MCPContent;
}

describe('message-document', () => {
  it('extracts presentInteractive raw-data JSON', () => {
    const html =
      '<div id="content-title">Flow</div><script id="raw-data" type="application/json">"A --> B\\n"</script><div id="md-root"></div>';
    expect(extractPresentInteractiveRawData(html)).toBe('A --> B\n');
  });

  it('resolves reportResult structured content into composed markdown', () => {
    const message = createMessage({
      role: 'tool',
      metadata: {
        structuredContent: {
          type: 'reportResult',
          status: 'success',
          title: 'Done',
          criteria: 'tests',
          proof: 'ci',
          result: 'shipped',
          deliverables: [],
        },
      },
    });

    const doc = resolveMessageDocument(message);
    expect(doc?.source).toBe('reportResult');
    expect(doc?.content).toContain('# Done');
    expect(doc?.content).toContain('## Outcome');
    expect(doc?.content).toContain('shipped');
  });

  it('resolves presentInteractive HTML resource body', () => {
    const html = [
      '<div id="content-title">Diagram</div>',
      '<script id="raw-data" type="application/json">"# Hi\\n"</script>',
      '<div id="md-root"></div>',
    ].join('');
    const message = createMessage({
      content: [presentInteractiveResource(html)],
    });

    const doc = resolveMessageDocument(message);
    expect(doc?.source).toBe('presentInteractive');
    expect(doc?.exportKind).toBe('markdown');
    expect(doc?.fileBaseName).toBe('Diagram');
    expect(doc?.content).toContain('# Hi');
  });

  it('feeds UI documents into text copy and download serializers', () => {
    const message = createMessage({
      role: 'tool',
      content: [{ type: 'text', text: 'wrapper summary STOP' }],
      metadata: {
        structuredContent: {
          type: 'reportResult',
          status: 'success',
          title: 'Done',
          result: 'body',
          deliverables: [],
        },
      },
    });

    expect(serializeMessageForClipboard(message, { mode: 'text' })).toContain(
      '# Done',
    );
    expect(serializeMessageForDownload(message)).toContain('body');
    expect(serializeMessageForDownload(message)).not.toContain('STOP');
  });
});
