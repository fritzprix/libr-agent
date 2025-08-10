import { useContext } from 'react';
import { UnifiedMCPContext } from '../context/UnifiedMCPContext';

/**
 * 🔧 Hook for using Unified MCP functionality
 *
 * Provides access to both Tauri MCP and Web Worker MCP systems through a single interface.
 * Must be used within a UnifiedMCPProvider.
 */
export const useUnifiedMCP = () => {
  const context = useContext(UnifiedMCPContext);

  if (context === undefined) {
    throw new Error('useUnifiedMCP must be used within a UnifiedMCPProvider');
  }

  return context;
};
