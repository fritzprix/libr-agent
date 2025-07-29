import React, { useMemo } from 'react';
import { useLocalTools } from '@/context/LocalToolContext';
import { useMCPServer } from '@/hooks/use-mcp-server';

interface ToolsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const ToolsModal: React.FC<ToolsModalProps> = ({ isOpen, onClose }) => {
  const { availableTools: mcpTools } = useMCPServer();
  const { availableTools: localTools } = useLocalTools();
  const availableTools = useMemo(
    () => [...mcpTools, ...localTools],
    [localTools, mcpTools],
  );
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-background border border-border rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80vh] overflow-hidden">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-foreground">
            Available Tools ({availableTools.length})
          </h2>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-destructive transition-colors"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div className="overflow-y-auto terminal-scrollbar max-h-[60vh]">
          {availableTools.length === 0 ? (
            <div className="text-muted-foreground text-center py-8">
              No MCP tools available. Configure MCP servers in Role Manager.
            </div>
          ) : (
            <div className="space-y-3">
              {availableTools.map((tool, index) => (
                <div
                  key={index}
                  className="bg-muted border border-border rounded p-3"
                >
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-accent font-mono text-sm">
                      🔧 {tool.name}
                    </span>
                  </div>
                  {tool.description && (
                    <p className="text-foreground text-sm">
                      {tool.description}
                    </p>
                  )}
                  {tool.inputSchema && (
                    <details className="mt-2">
                      <summary className="text-xs text-muted-foreground cursor-pointer hover:text-foreground">
                        Input Schema
                      </summary>
                      <pre className="text-xs text-muted-foreground mt-1 bg-background p-2 rounded overflow-x-auto">
                        {JSON.stringify(tool.inputSchema, null, 2)}
                      </pre>
                    </details>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="mt-4 pt-4 border-t border-border">
          <button
            onClick={onClose}
            className="w-full bg-accent hover:bg-accent/80 text-accent-foreground py-2 rounded transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

export default ToolsModal;
