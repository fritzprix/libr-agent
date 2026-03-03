import React, { useState, useEffect } from 'react';
import { Wrench, ChevronDown } from 'lucide-react';
import { safeInvoke } from '@/lib/backend/core';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp';

const logger = getLogger('ServerToolsModal');

interface ServerToolsModalProps {
  serverId: string;
  serverName: string;
  isOpen: boolean;
  onClose: () => void;
}

/**
 * Modal that fetches and displays all tools (with descriptions and schemas)
 * for a given MCP server by calling probe_mcp_server.
 */
export const ServerToolsModal: React.FC<ServerToolsModalProps> = ({
  serverId,
  serverName,
  isOpen,
  onClose,
}) => {
  const [tools, setTools] = useState<MCPTool[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    setIsLoading(true);
    setError(null);
    setTools([]);

    safeInvoke<MCPTool[]>('probe_mcp_server', { serverId })
      .then((result) => {
        setTools(result);
      })
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        logger.error('Failed to probe server tools', { serverId, err });
        setError(msg);
      })
      .finally(() => {
        setIsLoading(false);
      });
  }, [isOpen, serverId]);

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Wrench size={18} />
            {serverName}
            {tools.length > 0 && (
              <span className="text-muted-foreground font-normal text-sm">
                ({tools.length} tools)
              </span>
            )}
          </DialogTitle>
          <DialogDescription className="text-sm text-muted-foreground text-left">
            Tool names and descriptions as returned by the MCP server.
          </DialogDescription>
        </DialogHeader>

        {isLoading && (
          <div className="flex flex-col items-center justify-center py-10 gap-3 text-muted-foreground">
            <LoadingSpinner />
            <span className="text-sm">Connecting to server…</span>
          </div>
        )}

        {error && (
          <div className="py-8 text-center text-destructive text-sm">
            <p className="font-semibold mb-1">Failed to load tools</p>
            <p className="opacity-90">{error}</p>
          </div>
        )}

        {!isLoading && !error && (
          <div className="overflow-y-auto flex-1 min-h-0 pr-1">
            {tools.length === 0 ? (
              <p className="text-center py-8 text-muted-foreground text-sm">
                No tools returned.
              </p>
            ) : (
              <ul className="space-y-2" aria-label="Tool list">
                {tools.map((tool) => (
                  <li
                    key={tool.name}
                    className="bg-muted border border-border rounded p-3"
                  >
                    <p className="font-mono text-sm font-medium text-foreground">
                      {tool.title ? `${tool.title} (${tool.name})` : tool.name}
                    </p>
                    {tool.description && (
                      <p className="text-muted-foreground text-sm mt-1">
                        {tool.description}
                      </p>
                    )}
                    {tool.inputSchema && (
                      <details className="mt-2 group">
                        <summary className="text-xs text-primary cursor-pointer hover:underline inline-flex items-center gap-1 select-none">
                          <ChevronDown
                            size={12}
                            className="transition-transform group-open:rotate-180"
                          />
                          Input schema
                        </summary>
                        <pre className="text-xs text-foreground mt-1 bg-background p-2 rounded border border-border overflow-x-auto">
                          {JSON.stringify(tool.inputSchema, null, 2)}
                        </pre>
                      </details>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <div className="mt-4 pt-4 border-t border-border flex justify-end">
          <Button variant="secondary" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
};

export default ServerToolsModal;
