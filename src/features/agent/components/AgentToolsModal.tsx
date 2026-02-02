import React, { useMemo } from 'react';
import { Wrench } from 'lucide-react';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useAgentSessionState } from '@/context/AgentSessionContext';

interface AgentToolsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * AgentToolsModal - Tools list modal for Agent V2
 *
 * Differences from Legacy ToolsModal:
 * 1. Data source: useAgentTools(sessionId) - Filtered tools from Rust backend
 * 2. Context: AssistantContext → AgentSessionContext
 * 3. Disabled state: No disabled tools since backend filters
 * 4. Single source: UI and LLM display same tool list
 */
export const AgentToolsModal: React.FC<AgentToolsModalProps> = ({
  isOpen,
  onClose,
}) => {
  const { session } = useAgentSessionState();

  // ✅ Single Source of Truth: Filtered tools from Rust backend
  const { availableTools, isLoading, error } = useAgentTools(session?.id);

  // Categorize by type (builtin vs external MCP)
  const { builtinTools, mcpTools } = useMemo(() => {
    const builtin = availableTools.filter((t) => t.name.startsWith('builtin_'));
    const mcp = availableTools.filter((t) => !t.name.startsWith('builtin_'));
    return { builtinTools: builtin, mcpTools: mcp };
  }, [availableTools]);

  const totalCount = availableTools.length;
  const builtinCount = builtinTools.length;
  const mcpCount = mcpTools.length;

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-background border border-border rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80%] overflow-hidden flex flex-col">
        <div className="flex items-center justify-between mb-4 flex-shrink-0">
          <h2 className="text-lg font-bold text-foreground">
            Available Tools {totalCount}({mcpCount})
          </h2>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-destructive transition-colors"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        {builtinCount > 0 && (
          <div className="text-sm text-muted-foreground mb-4">
            Built-in tools enabled: {builtinCount}
          </div>
        )}

        {/* Loading State */}
        {isLoading && (
          <div className="text-center py-8 text-muted-foreground">
            Loading tools...
          </div>
        )}

        {/* Error State */}
        {error && (
          <div className="text-center py-8 text-destructive">
            Error loading tools: {error}
          </div>
        )}

        {/* Tools List */}
        {!isLoading && !error && (
          <div className="overflow-y-auto flex-1 min-h-0">
            {totalCount === 0 ? (
              <div className="text-foreground text-center py-8">
                No tools available for this agent session.
              </div>
            ) : (
              <div className="space-y-3">
                {availableTools.map((tool) => (
                  <div
                    key={tool.name}
                    className="bg-muted border border-border rounded p-3"
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <Wrench size={16} className="flex-shrink-0" />
                        <span
                          className="font-mono text-sm text-foreground break-words"
                          title={tool.name}
                        >
                          {tool.name}
                        </span>
                        <span
                          className={
                            tool.name.startsWith('builtin_')
                              ? 'text-xs bg-success text-success-foreground px-2 py-0.5 rounded-full'
                              : 'text-xs bg-primary text-primary-foreground px-2 py-0.5 rounded-full'
                          }
                          aria-hidden
                        >
                          {tool.name.startsWith('builtin_') ? 'builtin' : 'mcp'}
                        </span>
                      </div>
                    </div>
                    {tool.description && (
                      <p className="text-foreground text-sm">
                        {tool.description}
                      </p>
                    )}
                    {tool.inputSchema && (
                      <details className="mt-2">
                        <summary className="text-xs text-foreground/80 cursor-pointer hover:text-foreground">
                          Input Schema
                        </summary>
                        <pre className="text-xs text-foreground mt-1 bg-background p-2 rounded overflow-x-auto">
                          {JSON.stringify(tool.inputSchema, null, 2)}
                        </pre>
                      </details>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

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

export default AgentToolsModal;
