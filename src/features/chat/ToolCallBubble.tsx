import React from 'react';
import { Wrench } from 'lucide-react';
import { BaseBubble } from '@/components/ui/BaseBubble';
import { JsonViewer } from '@/components/ui/JsonViewer';

interface ToolCallBubbleProps {
  tool_calls: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }[];
}

const ToolCallBubble: React.FC<ToolCallBubbleProps> = ({ tool_calls }) => {
  const badge = (
    <span className="px-2 py-1 bg-primary text-primary-foreground text-xs rounded-full">
      {tool_calls.length} call{tool_calls.length !== 1 ? 's' : ''}
    </span>
  );

  const collapsedSummary = (
    <div className="flex flex-wrap gap-2">
      {tool_calls.map((call, index) => (
        <span
          key={index}
          className="inline-flex items-center gap-1 px-2 py-1 bg-muted rounded-full text-xs"
        >
          <span className="text-primary">{call.function.name}</span>
        </span>
      ))}
    </div>
  );

  const copyData = JSON.stringify(tool_calls, null, 2);

  return (
    <BaseBubble
      title="Tool Call"
      icon={<Wrench size={16} />}
      badge={badge}
      copyData={copyData}
      collapsedSummary={collapsedSummary}
    >
      {tool_calls.map((call) => {
        let parsedArgs = null;
        try {
          parsedArgs = JSON.parse(call.function.arguments);
        } catch {
          // arguments가 JSON이 아닌 경우 그대로 사용
        }

        return (
          <div key={call.id} className="mb-4 last:mb-0">
            <div className="flex items-center gap-2 mb-2">
              <span className="font-mono text-sm font-medium text-primary">
                {call.function.name}
              </span>
              <span className="text-xs text-muted-foreground">
                #{call.id}
              </span>
            </div>
            <div className="pl-4 border-l-2 border-border">
              {parsedArgs ? (
                <JsonViewer data={parsedArgs} />
              ) : (
                <pre className="text-sm text-foreground font-mono whitespace-pre-wrap break-words">
                  {call.function.arguments}
                </pre>
              )}
            </div>
          </div>
        );
      })}
    </BaseBubble>
  );
};

export default ToolCallBubble;
