import React from 'react';
import { Plus } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { McpServerService } from '@/lib/services/mcp-server-service';
import { Button } from '@/components/ui';
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
import { MCPServerDialog } from './MCPServerDialog';
import { useMCPServerManagement } from './hooks/useMCPServerManagement';
import { ServerCard } from './components/ServerCard';
import { RecommendedPresets } from './components/RecommendedPresets';

interface MCPServerManagementProps {
  service?: McpServerService;
}

function MCPServerManagementComponent({ service }: MCPServerManagementProps) {
  const { t } = useTranslation('common');

  const {
    servers,
    presets,
    isLoading,
    isValidating,
    hasNextPage,
    setSize,
    editingServer,
    setEditingServer,
    serverToDelete,
    setServerToDelete,
    verificationStatus,
    handleCreateNew,
    handleSetupPreset,
    handleSave,
    handleDelete,
    confirmDelete,
    handleToggleActive,
  } = useMCPServerManagement(service);

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h2 className="text-xl font-semibold">
          {t('mcpServer.title', 'Extensions')}
        </h2>
        <Button onClick={handleCreateNew}>
          <Plus className="w-4 h-4 mr-2" />
          {t('mcpServer.addServer', 'Add Extension')}
        </Button>
      </div>

      {/* Recommended Servers Section */}
      <RecommendedPresets
        presets={presets}
        servers={servers}
        onSetupPreset={handleSetupPreset}
      />

      {/* Existing Servers List */}
      <div className="space-y-4">
        {isLoading ? (
          <div className="text-center py-8 text-muted-foreground">
            {t('mcpServer.loading', 'Loading extensions...')}
          </div>
        ) : servers.length === 0 ? (
          <div className="text-center py-8 text-muted-foreground border-2 border-dashed rounded-lg">
            {t(
              'mcpServer.noServers',
              'No extensions installed. Add one or choose a recommended extension above.',
            )}
          </div>
        ) : (
          <>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {servers.map((server) => (
                <ServerCard
                  key={server.id}
                  server={server}
                  onEdit={setEditingServer}
                  onDelete={handleDelete}
                  onToggleActive={handleToggleActive}
                  verificationStatus={verificationStatus[server.id]}
                />
              ))}
            </div>

            {isValidating && servers.length > 0 && (
              <div className="flex justify-center py-2">
                <span className="text-xs text-muted-foreground">
                  {t('mcpServer.updating', 'Updating...')}
                </span>
              </div>
            )}

            {hasNextPage && (
              <div className="flex justify-center pt-2">
                <Button
                  variant="outline"
                  disabled={isValidating}
                  onClick={() => setSize((s) => s + 1)}
                >
                  {isValidating
                    ? t('mcpServer.loadingMore', 'Loading...')
                    : t('mcpServer.loadMore', 'Load more')}
                </Button>
              </div>
            )}
          </>
        )}
      </div>

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
            <AlertDialogTitle>
              {t('mcpServer.deleteDialog.title', 'Delete Extension')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('mcpServer.deleteDialog.description', {
                name: serverToDelete?.name,
                defaultValue:
                  'Are you sure you want to delete "{{name}}"? This action cannot be undone.',
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>
              {t('mcpServer.deleteDialog.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction onClick={confirmDelete}>
              {t('mcpServer.deleteDialog.confirm', 'Delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

// Memoize entire component - only re-render when explicitly needed
export const MCPServerManagement = React.memo(MCPServerManagementComponent);
