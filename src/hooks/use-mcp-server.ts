import { useContext } from 'react';
import { MCPServerContext } from '../context/MCPServerContext';

export const useMCPServer = () => {
  const context = useContext(MCPServerContext);
  if (context === undefined) {
    throw new Error('useMCPServer must be used within a MCPServerProvider');
  }
  return context;
};
