
import React from 'react';

interface ToolCallBubbleProps {
  tool_calls: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }[];
}

const ToolCallBubble: React.FC<ToolCallBubbleProps> = ({ tool_calls }) => {
  return (
    <div className="mt-4 p-3 bg-popover rounded-lg border border-border">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-sm">🛠️</span>
        <span className="text-sm font-medium">Tool Call</span>
      </div>
      <div className="flex flex-wrap gap-2">
        {tool_calls.map((tool_call, index) => (
          <div
            key={index}
            className="inline-flex items-center gap-2 px-3 py-1 bg-muted rounded-full text-xs"
          >
            {tool_call.function && (
              <>
                <span className="text-primary">
                  {tool_call.function.name}
                </span>
                <span className="truncate max-w-32">
                  {tool_call.function.arguments}
                </span>
              </>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

export default ToolCallBubble;
