import React, { useMemo, useState, useCallback } from 'react';
import useSWRInfinite from 'swr/infinite';
import { Plus } from 'lucide-react';
import { createId } from '@paralleldrive/cuid2';
import { MCPServerEntity } from '@/models/chat';
import { McpServerService } from '@/lib/services/mcp-server-service';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { useSettings } from '@/hooks/use-settings';
import {
  Button,
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from '@/components/ui';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Switch } from '@/components/ui/switch';
import { MCPServerDialog } from './MCPServerDialog';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MCPServerManagement');

// Memoized ServerCard component to prevent unnecessary re-renders
interface ServerCardProps {
  server: MCPServerEntity;
  onEdit: (server: MCPServerEntity) => void;
  onDelete: (server: MCPServerEntity) => void;
  onToggleActive: (server: MCPServerEntity, checked: boolean) => void;
}

const ServerCard = React.memo(
  ({ server, onEdit, onDelete, onToggleActive }: ServerCardProps) => {
    return (
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div className="flex-1">
            <CardTitle className="text-base">
              {server.name || 'Unnamed Server'}
            </CardTitle>
            <p className="text-sm text-muted-foreground mt-1">
              {server.metadata?.description || 'No description'}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              Transport: {server.transport.type}
              {server.transport.type === 'stdio' &&
                ` • ${server.transport.command}`}
              {((server.transport.type as string) === 'http' ||
                server.transport.type === 'http-sse') &&
                ` • ${(server.transport as { url: string }).url}`}
            </p>
            {server.toolCount !== undefined && server.toolCount !== null && (
              <p className="text-xs text-muted-foreground mt-1">
                {server.toolCount} tool{server.toolCount !== 1 ? 's' : ''}{' '}
                available
              </p>
            )}
            {(server.toolCount === undefined || server.toolCount === null) && (
              <p className="text-xs text-muted-foreground italic mt-1">
                Tool count unknown (not yet verified)
              </p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <div className="flex flex-col items-end gap-1">
              <span className="text-xs text-muted-foreground">Active</span>
              <Switch
                checked={server.isActive}
                onCheckedChange={(checked) => onToggleActive(server, checked)}
              />
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => onEdit(server)}>
              Edit
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => onDelete(server)}
            >
              Delete
            </Button>
          </div>
        </CardContent>
      </Card>
    );
  },
  (prev, next) => {
    return (
      prev.server.id === next.server.id &&
      prev.server.name === next.server.name &&
      prev.server.isActive === next.server.isActive &&
      prev.server.updatedAt?.getTime() === next.server.updatedAt?.getTime() &&
      prev.onEdit === next.onEdit &&
      prev.onDelete === next.onDelete &&
      prev.onToggleActive === next.onToggleActive
    );
  },
);

ServerCard.displayName = 'ServerCard';

function MCPServerManagementComponent() {
  const { saveServer, deleteServer, toggleActive } = useMCPServerRegistry();
  const { value: settings } = useSettings();

  const mcpServerService = useMemo(() => {
    return new McpServerService(settings.agentHubUrl);
  }, [settings.agentHubUrl]);

  // Follow SessionContext pattern: useSWRInfinite + Page<T>
  const {
    data,
    isLoading,
    isValidating,
    setSize,
    mutate: mutateServers,
  } = useSWRInfinite(
    (pageIndex) => ['mcpServers', pageIndex],
    async ([, pageIndex]) => {
      // getPage is 1-based; pass pageIndex + 1
      return mcpServerService.getPage(pageIndex + 1, 10);
    },
    {
      revalidateOnFocus: false,
      revalidateOnReconnect: false,
      dedupingInterval: 2000, // Dedupe requests within 2 seconds
      keepPreviousData: true, // Keep previous data while loading new
    },
  );

  const pages = data ?? [];
  const servers = useMemo(() => pages.flatMap((p) => p.items), [pages]);
  const hasNextPage = useMemo(
    () => !(pages.length > 0 && !pages[pages.length - 1].hasNextPage),
    [pages],
  );

  const [editingServer, setEditingServer] = useState<MCPServerEntity | null>(
    null,
  );
  const [serverToDelete, setServerToDelete] = useState<MCPServerEntity | null>(
    null,
  );

  const handleCreateNew = useCallback(() => {
    const newServer: MCPServerEntity = {
      id: createId(),
      name: '',
      isActive: true,
      createdAt: new Date(),
      updatedAt: new Date(),
      transport: {
        type: 'stdio',
        command: '',
        args: [],
      },
    };
    setEditingServer(newServer);
  }, []);

  const handleEdit = useCallback((server: MCPServerEntity) => {
    setEditingServer(server);
  }, []);

  const handleSave = useCallback(
    async (server: MCPServerEntity) => {
      try {
        await saveServer(server);
        await mutateServers();
        setEditingServer(null);
        toast.success('MCP server saved successfully');
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Unknown error';
        toast.error(`Failed to save server: ${message}`);
        logger.error('Failed to save MCP server', error);
      }
    },
    [saveServer, mutateServers],
  );

  const handleDelete = useCallback((server: MCPServerEntity) => {
    setServerToDelete(server);
  }, []);

  const confirmDelete = useCallback(async () => {
    if (!serverToDelete) return;

    try {
      await deleteServer(serverToDelete.id);
      await mutateServers();
      toast.success('MCP server deleted successfully');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to delete server: ${message}`);
      logger.error('Failed to delete MCP server', error);
    } finally {
      setServerToDelete(null);
    }
  }, [serverToDelete, deleteServer, mutateServers]);

  const handleToggleActive = useCallback(
    async (server: MCPServerEntity, checked: boolean) => {
      try {
        await toggleActive(server.id, checked);
        await mutateServers();
        toast.success(`MCP server ${checked ? 'activated' : 'deactivated'}`);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Unknown error';
        toast.error(`Failed to toggle server: ${message}`);
        logger.error('Failed to toggle MCP server active status', error);
      }
    },
    [toggleActive, mutateServers],
  );

  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <h2 className="text-xl font-semibold">MCP Server Management</h2>
        <Button onClick={handleCreateNew}>
          <Plus className="w-4 h-4 mr-2" />
          Add Server
        </Button>
      </div>

      {isLoading ? (
        <div className="text-center py-8 text-muted-foreground">
          Loading MCP servers...
        </div>
      ) : servers.length === 0 ? (
        <div className="text-center py-8 text-muted-foreground">
          No MCP servers configured. Click &ldquo;Add Server&rdquo; to create
          one.
        </div>
      ) : (
        <>
          <div className="grid gap-4">
            {servers.map((server) => (
              <ServerCard
                key={server.id}
                server={server}
                onEdit={handleEdit}
                onDelete={handleDelete}
                onToggleActive={handleToggleActive}
              />
            ))}
          </div>

          {hasNextPage && (
            <div className="flex justify-center pt-2">
              <Button
                variant="outline"
                disabled={isValidating}
                onClick={() => setSize((s) => s + 1)}
              >
                {isValidating ? 'Loading…' : 'Load more'}
              </Button>
            </div>
          )}
        </>
      )}

      {editingServer && (
        <MCPServerDialog
          server={editingServer}
          onSave={handleSave}
          onCancel={() => setEditingServer(null)}
        />
      )}

      <AlertDialog
        open={!!serverToDelete}
        onOpenChange={(open) => !open && setServerToDelete(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete MCP Server</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete &quot;{serverToDelete?.name}
              &quot;? This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={confirmDelete}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

// Memoize entire component - only re-render when explicitly needed
export const MCPServerManagement = React.memo(MCPServerManagementComponent);
