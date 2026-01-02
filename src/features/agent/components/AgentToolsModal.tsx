import React, { useMemo } from 'react';
import { Wrench } from 'lucide-react';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useAgentSessionState } from '@/context/AgentSessionContext';

interface AgentToolsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * AgentToolsModal - Agent V2용 도구 목록 모달
 *
 * Legacy ToolsModal과의 차이점:
 * 1. 데이터 소스: useAgentTools(sessionId) - Rust 백엔드에서 필터링된 도구
 * 2. Context: AssistantContext → AgentSessionContext
 * 3. Disabled 상태: 백엔드가 이미 필터링하므로 disabled 도구 없음
 * 4. 단일 정보 소스: UI와 LLM이 동일한 도구 리스트 표시
 */
export const AgentToolsModal: React.FC<AgentToolsModalProps> = ({
  isOpen,
  onClose,
}) => {
  const { session } = useAgentSessionState();

  // ✅ Single Source of Truth: Rust 백엔드에서 필터링된 도구
  const { availableTools, isLoading, error } = useAgentTools(session?.id);

  // 카테고리별 분류 (builtin vs external MCP)
  const { builtinTools, mcpTools } = useMemo(() => {
    const builtin = availableTools.filter((t) =>
      t.name.startsWith('builtin_'),
    );
    const mcp = availableTools.filter((t) => !t.name.startsWith('builtin_'));
    return { builtinTools: builtin, mcpTools: mcp };
  }, [availableTools]);

  const totalCount = availableTools.length;
  const builtinCount = builtinTools.length;
  const mcpCount = mcpTools.length;

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-background border border-border rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80vh] overflow-hidden">
        <div className="flex items-center justify-between mb-4">
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
          <div className="text-center py-8 text-red-500">
            Error loading tools: {error}
          </div>
        )}

        {/* Tools List */}
        {!isLoading && !error && (
          <div className="overflow-y-auto terminal-scrollbar max-h-[60vh]">
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
                              ? 'text-xs bg-emerald-600 text-emerald-foreground px-2 py-0.5 rounded-full'
                              : 'text-xs bg-sky-600 text-sky-foreground px-2 py-0.5 rounded-full'
                          }
                          aria-hidden
                        >
                          {tool.name.startsWith('builtin_')
                            ? 'builtin'
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
