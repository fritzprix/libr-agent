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

  it('resolves reportResult from toolResultsMap via displayContent tool_call ids', () => {
    const assistant = createMessage({
      id: 'asst-1',
      content: [
        { type: 'text', text: 'FIRST RESPONSE: I will analyze the repo...' },
      ],
      tool_calls: [
        {
          id: 'call-search',
          type: 'function',
          function: { name: 'workspace__runShell', arguments: '{}' },
        },
      ],
    });

    const displayContent: MCPContent[] = [
      { type: 'text', text: 'FIRST RESPONSE: I will analyze the repo...' },
      {
        type: 'tool_call',
        id: 'call-search',
        name: 'workspace__runShell',
        arguments: '{}',
      },
      {
        type: 'tool_call',
        id: 'call-report',
        name: 'ui__reportResult',
        arguments: '{}',
      },
    ];

    const toolResultsMap = new Map<string, Message>([
      [
        'call-search',
        createMessage({
          id: 'tool-search',
          role: 'tool',
          tool_call_id: 'call-search',
          content: [{ type: 'text', text: 'ls ok' }],
        }),
      ],
      [
        'call-report',
        createMessage({
          id: 'tool-report',
          role: 'tool',
          tool_call_id: 'call-report',
          content: [{ type: 'text', text: 'Final result reported STOP' }],
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'success',
              title: 'Ship',
              result: 'REPORT BODY',
              deliverables: [],
            },
          },
        }),
      ],
    ]);

    const doc = resolveMessageDocument(assistant, {
      displayContent,
      toolResultsMap,
    });
    expect(doc?.source).toBe('reportResult');
    expect(doc?.content).toContain('REPORT BODY');
    expect(doc?.content).not.toContain('FIRST RESPONSE');

    expect(
      serializeMessageForClipboard(assistant, {
        mode: 'text',
        displayContent,
        toolResultsMap,
      }),
    ).toContain('REPORT BODY');
    expect(
      serializeMessageForClipboard(assistant, {
        mode: 'text',
        displayContent,
        toolResultsMap,
      }),
    ).not.toContain('FIRST RESPONSE');

    expect(
      serializeMessageForDownload(assistant, {
        displayContent,
        toolResultsMap,
      }),
    ).toContain('REPORT BODY');
  });

  it('does not use an unrelated session reportResult outside this bubble', () => {
    const assistant = createMessage({
      content: [{ type: 'text', text: 'Visible narration' }],
      tool_calls: [
        {
          id: 'call-local',
          type: 'function',
          function: { name: 'workspace__runShell', arguments: '{}' },
        },
      ],
    });

    const toolResultsMap = new Map<string, Message>([
      [
        'call-local',
        createMessage({
          id: 'tool-local',
          role: 'tool',
          tool_call_id: 'call-local',
          content: [{ type: 'text', text: 'ok' }],
        }),
      ],
      [
        'call-other-report',
        createMessage({
          id: 'tool-other',
          role: 'tool',
          tool_call_id: 'call-other-report',
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'success',
              title: 'Other',
              result: 'OTHER SESSION RESULT',
              deliverables: [],
            },
          },
        }),
      ],
    ]);

    expect(
      resolveMessageDocument(assistant, { toolResultsMap }),
    ).toBeNull();
    expect(
      serializeMessageForClipboard(assistant, {
        mode: 'text',
        toolResultsMap,
      }),
    ).toBe('Visible narration');
    expect(
      serializeMessageForDownload(assistant, { toolResultsMap }),
    ).toBe('Visible narration');
  });

  it('prefers the newest reportResult in the bubble when several exist', () => {
    const assistant = createMessage({
      content: [{ type: 'text', text: 'Working...' }],
    });
    const displayContent: MCPContent[] = [
      { type: 'text', text: 'Working...' },
      {
        type: 'tool_call',
        id: 'call-report-1',
        name: 'ui__reportResult',
        arguments: '{}',
      },
      {
        type: 'tool_call',
        id: 'call-report-2',
        name: 'ui__reportResult',
        arguments: '{}',
      },
    ];
    const toolResultsMap = new Map<string, Message>([
      [
        'call-report-1',
        createMessage({
          id: 'tool-1',
          role: 'tool',
          tool_call_id: 'call-report-1',
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'partial',
              title: 'First',
              result: 'OLD BODY',
              deliverables: [],
            },
          },
        }),
      ],
      [
        'call-report-2',
        createMessage({
          id: 'tool-2',
          role: 'tool',
          tool_call_id: 'call-report-2',
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'success',
              title: 'Final',
              result: 'NEW BODY',
              deliverables: [],
            },
          },
        }),
      ],
    ]);

    const doc = resolveMessageDocument(assistant, {
      displayContent,
      toolResultsMap,
    });
    expect(doc?.content).toContain('NEW BODY');
    expect(doc?.content).not.toContain('OLD BODY');
  });
});
