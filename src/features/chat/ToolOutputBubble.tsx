import React from 'react';
import { Message } from '@/models/chat';
import { BaseBubble } from '@/components/ui/BaseBubble';
import { JsonViewer } from '@/components/ui/JsonViewer';
import MessageRenderer from '@/components/MessageRenderer';

interface ToolOutputBubbleProps {
  message: Message;
  defaultExpanded?: boolean;
}

export const ToolOutputBubble: React.FC<ToolOutputBubbleProps> = ({
  message,
  defaultExpanded = false,
}) => {
  const { content } = message;

  // If content is MCPContent array, use MessageRenderer directly
  if (Array.isArray(content)) {
    return (
      <BaseBubble
        title="Tool Output"
        badge={
          <span className="px-2 py-1 bg-primary text-primary-foreground text-xs rounded-full">
            MCP
          </span>
        }
        defaultExpanded={defaultExpanded}
        copyData={JSON.stringify(content, null, 2)}
        collapsedSummary={<span>{content.length} content items</span>}
      >
        <MessageRenderer content={content} className="text-sm" />
      </BaseBubble>
    );
  }

  const stringContent = typeof content === 'string' ? content : '';
  const parsedContent = (() => {
    try {
      return JSON.parse(stringContent);
    } catch {
      return null;
    }
  })();

  const isJson = parsedContent !== null;

  const badge = isJson ? (
    <span className="px-2 py-1 bg-primary text-primary-foreground text-xs rounded-full">
      JSON
    </span>
  ) : null;

  const collapsedSummary = isJson ? (
    <span>
      {Array.isArray(parsedContent)
        ? `Array with ${parsedContent.length} items`
        : typeof parsedContent === 'object' && parsedContent !== null
          ? `Object with ${Object.keys(parsedContent).length} keys`
          : `${typeof parsedContent} value`}
    </span>
  ) : (
    <span>{stringContent.length} characters</span>
  );

  const copyData = isJson
    ? JSON.stringify(parsedContent, null, 2)
    : stringContent;

  return (
    <BaseBubble
      title="Tool Output"
      badge={badge}
      defaultExpanded={defaultExpanded}
      copyData={copyData}
      collapsedSummary={collapsedSummary}
    >
      {isJson ? (
        <div className="text-sm">
          <JsonViewer data={parsedContent} />
        </div>
      ) : (
        <pre className="text-sm text-foreground font-mono whitespace-pre-wrap break-words">
          {stringContent}
        </pre>
      )}
    </BaseBubble>
  );
};

export default ToolOutputBubble;
