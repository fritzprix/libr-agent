import React, { useMemo } from 'react';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useBuiltInTool } from '.';
import { MCPTool } from '@/lib/mcp-types';
import { useAssistantContext } from '@/context/AssistantContext';
import { extractBuiltInServiceAlias } from '@/lib/utils';

interface ToolsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const ToolsModal: React.FC<ToolsModalProps> = ({ isOpen, onClose }) => {
  const { availableTools: mcpTools } = useMCPServer();
  const { availableTools: builtinTools } = useBuiltInTool();
  const { currentAssistant } = useAssistantContext();

  type ToolSource = 'mcp' | 'builtin';
  type DisplayTool = MCPTool & {
    source: ToolSource;
    disabled?: boolean;
    serviceAlias?: string | null;
  };

  const allowedAliases = currentAssistant?.allowedBuiltInServiceAliases;

  const builtinWithState: DisplayTool[] = useMemo(() => {
    return builtinTools.map((tool) => {
      const alias = extractBuiltInServiceAlias(tool.name);
      const disabled =
        allowedAliases === undefined
          ? false
          : !(alias && allowedAliases.includes(alias));

      return {
        ...tool,
        source: 'builtin' as const,
        disabled,
        serviceAlias: alias,
      };
    });
  }, [allowedAliases, builtinTools]);

  const mcpWithState: DisplayTool[] = useMemo(
    () => mcpTools.map((tool) => ({ ...tool, source: 'mcp' as const })),
    [mcpTools],
  );

  const availableTools: DisplayTool[] = useMemo(() => {
    const combined = [...builtinWithState, ...mcpWithState];
    // show builtin first then mcp, sorted by name for stability
    return combined.sort((a, b) => a.name.localeCompare(b.name));
  }, [builtinWithState, mcpWithState]);

  const enabledBuiltinCount = builtinWithState.filter((t) => !t.disabled).length;
  const totalBuiltinCount = builtinWithState.length;
  const mcpCount = mcpWithState.length;
  const accessibleCount = enabledBuiltinCount + mcpCount;

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-background border border-border rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80vh] overflow-hidden">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-foreground">
            Available Tools {accessibleCount}({mcpCount})
          </h2>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-destructive transition-colors"
            aria-label="Close"
          >
            ✕
          </button>
        </div>
        {totalBuiltinCount > 0 && (
          <div className="text-sm text-muted-foreground mb-4">
            Built-in tools enabled {enabledBuiltinCount}/{totalBuiltinCount}
          </div>
        )}

        <div className="overflow-y-auto terminal-scrollbar max-h-[60vh]">
          {availableTools.length === 0 ? (
            <div className="text-foreground text-center py-8">
              No tools available. Configure MCP servers or enable built-in
              tools.
            </div>
          ) : (
            <div className="space-y-3">
              {availableTools.map((tool) => (
                <div
                  key={tool.name}
                  className={`bg-muted border border-border rounded p-3 ${
                    tool.disabled ? 'opacity-60' : ''
                  }`}
                >
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <span
                        className="font-mono text-sm text-foreground break-words"
                        title={tool.name}
                      >
                        🔧 {tool.name}
                      </span>
                      <span
                        className={
                          tool.source === 'builtin'
                            ? 'text-xs bg-emerald-600 text-emerald-foreground px-2 py-0.5 rounded-full'
                            : 'text-xs bg-sky-600 text-sky-foreground px-2 py-0.5 rounded-full'
                        }
                        aria-hidden
                      >
                        {tool.source === 'builtin'
                          ? tool.disabled
                            ? 'builtin (disabled)'
                            : 'builtin'
                          : 'mcp'}
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
                  {tool.disabled && (
                    <div className="text-xs text-muted-foreground mt-2">
                      Disabled for {currentAssistant?.name ?? 'this assistant'}.
                      Enable it in the assistant settings.
                    </div>
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
