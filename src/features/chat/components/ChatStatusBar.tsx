import React from 'react';
import { CompactModelPicker } from '@/components/ui';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useBuiltInTool } from '@/features/tools';

interface ChatStatusBarProps {
  children?: React.ReactNode;
  onShowTools?: () => void;
}

export function ChatStatusBar({ children, onShowTools }: ChatStatusBarProps) {
  const { availableTools, isLoading, error } = useMCPServer();
  const { availableTools: builtinAvailable } = useBuiltInTool();

  const LoadingSpinner = () => (
    <svg
      className="animate-spin h-3 w-3 text-yellow-400"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );

  const getToolsDisplayText = () => {
    if (isLoading) return 'Loading tools...';
    if (error) return 'Tools error';
    const mcpCount = availableTools.length;
    const totalCount = mcpCount + (builtinAvailable?.length || 0);
    return `${totalCount}(${mcpCount}) available`;
  };

  const getToolsColor = () => {
    if (isLoading) return 'text-yellow-400';
    if (error) return 'text-red-400';
    const totalCount = availableTools.length + (builtinAvailable?.length || 0);
    return totalCount > 0 ? 'text-green-400' : 'text-gray-500';
  };

  const getToolsIcon = () => {
    if (isLoading) return <LoadingSpinner />;
    if (error) return '⚠️';
    return '🔧';
  };

  return (
    <div className="px-4 py-2 border-t flex items-center justify-between">
      <div>
        <CompactModelPicker />
        {children}
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs">Tools:</span>
        <button
          onClick={onShowTools}
          className={`text-xs transition-colors flex items-center gap-1 ${getToolsColor()}`}
          disabled={isLoading}
          title={error || undefined}
        >
          {getToolsIcon()} {getToolsDisplayText()}
        </button>
      </div>
    </div>
  );
}
