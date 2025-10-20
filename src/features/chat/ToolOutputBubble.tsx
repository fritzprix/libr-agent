import React from 'react';
import { Message } from '@/models/chat';
import { BaseBubble } from '@/components/ui/BaseBubble';
import { JsonViewer } from '@/components/ui/JsonViewer';
import MessageRenderer from '@/components/MessageRenderer';
import { separateContent } from '@/lib/content-utils';

interface ToolOutputBubbleProps {
  message: Message;
  defaultExpanded?: boolean;
}

export const ToolOutputBubble: React.FC<ToolOutputBubbleProps> = ({
  message,
  defaultExpanded = false,
}) => {
  const { content } = message;

  // If content is MCPContent array, separate UI and text content
  if (Array.isArray(content)) {
    const { uiContent, textContent } = separateContent(content);

    return (
      <div className="space-y-4">
        {/* UI Resource is always displayed */}
        {uiContent.length > 0 && (
          <MessageRenderer
            content={uiContent}
            className="text-sm"
            expandResources
          />
        )}

        {/* Text is collapsible */}
        {textContent.length > 0 && (
          <BaseBubble
            title="Tool Output Details"
            badge={
              <span className="px-2 py-1 bg-primary text-primary-foreground text-xs rounded-full">
                MCP
              </span>
            }
            defaultExpanded={defaultExpanded}
            copyData={JSON.stringify(textContent, null, 2)}
            collapsedSummary={<span>{textContent.length} content items</span>}
          >
            <MessageRenderer content={textContent} className="text-sm" />
          </BaseBubble>
        )}
      </div>
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
