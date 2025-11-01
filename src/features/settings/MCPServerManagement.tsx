import { useMemo, useState } from 'react';
import useSWRInfinite from 'swr/infinite';
import { Plus } from 'lucide-react';
import { createId } from '@paralleldrive/cuid2';
import { MCPServerEntity } from '@/models/chat';
import { dbService } from '@/lib/db/service';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import {
  Button,
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import { MCPServerDialog } from './MCPServerDialog';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MCPServerManagement');

export function MCPServerManagement() {
  const { saveServer, deleteServer, toggleActive } = useMCPServerRegistry();

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
      return dbService.mcpServers.getPage(pageIndex + 1, 10);
    },
    {
      revalidateOnFocus: false,
      revalidateOnReconnect: false,
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

  const handleCreateNew = () => {
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
  };

  const handleEdit = (server: MCPServerEntity) => {
    setEditingServer(server);
  };

  const handleSave = async (server: MCPServerEntity) => {
    try {
      await saveServer(server);
      await mutateServers();
      setEditingServer(null);
      toast.success('MCP server saved successfully');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to save server: ${message}`);
      logger.error('Failed to save MCP server', error);
    }
  };

  const handleDelete = async (server: MCPServerEntity) => {
    if (!confirm(`Delete MCP server "${server.name}"?`)) {
      return;
    }

    try {
      await deleteServer(server.id);
      await mutateServers();
      toast.success('MCP server deleted successfully');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to delete server: ${message}`);
      logger.error('Failed to delete MCP server', error);
    }
  };

  const handleToggleActive = async (
    server: MCPServerEntity,
    checked: boolean,
  ) => {
    try {
      await toggleActive(server.id, checked);
      await mutateServers();
      toast.success(`MCP server ${checked ? 'activated' : 'deactivated'}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to toggle server: ${message}`);
      logger.error('Failed to toggle MCP server active status', error);
    }
  };

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
              <Card key={server.id}>
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
                      {server.transport.type === 'http' &&
                        ` • ${server.transport.url}`}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="flex flex-col items-end gap-1">
                      <span className="text-xs text-muted-foreground">
                        Active
                      </span>
                      <Switch
                        checked={server.isActive}
                        onCheckedChange={(checked) =>
                          handleToggleActive(server, checked)
                        }
                      />
                    </div>
                  </div>
                </CardHeader>
                <CardContent>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleEdit(server)}
                    >
                      Edit
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => handleDelete(server)}
                    >
                      Delete
                    </Button>
                  </div>
                </CardContent>
              </Card>
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
    </div>
  );
}
