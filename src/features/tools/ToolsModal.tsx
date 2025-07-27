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
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-gray-900 border border-green-400 rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80vh] overflow-hidden">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-primary">
            Available Tools ({availableTools.length})
          </h2>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-red-400 transition-colors"
          >
            ✕
          </button>
        </div>

        <div className="overflow-y-auto terminal-scrollbar max-h-[60vh]">
          {availableTools.length === 0 ? (
            <div className="text-gray-400 text-center py-8">
              No MCP tools available. Configure MCP servers in Role Manager.
            </div>
          ) : (
            <div className="space-y-3">
              {availableTools.map((tool, index) => (
                <div
                  key={index}
                  className="bg-gray-800 border border-gray-700 rounded p-3"
                >
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-accent font-mono text-sm">
                      🔧 {tool.name}
                    </span>
                  </div>
                  {tool.description && (
                    <p className="text-gray-300 text-sm">{tool.description}</p>
                  )}
                  {tool.input_schema && (
                    <details className="mt-2">
                      <summary className="text-xs text-gray-400 cursor-pointer hover:text-gray-300">
                        Input Schema
                      </summary>
                      <pre className="text-xs text-gray-500 mt-1 bg-gray-900 p-2 rounded overflow-x-auto">
                        {JSON.stringify(tool.input_schema, null, 2)}
                      </pre>
                    </details>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="mt-4 pt-4 border-t border-gray-700">
          <button
            onClick={onClose}
            className="w-full bg-green-600 hover:bg-green-500 text-white py-2 rounded transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

export default ToolsModal;
