import React from 'react';
import { Plus, Package } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { McpServerService } from '@/lib/services/mcp-server-service';
import { Button, Separator } from '@/components/ui';
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
import LoadingSpinner from '@/components/ui/LoadingSpinner';
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
    allServers,
    registryLoaded,
    presets,
    isLoading,
    isValidating,
    hasNextPage,
    setSize,
    editingServer,
    setEditingServer,
    serverToDelete,
    setServerToDelete,
    isDeleting,
    togglingStatus,
    handleCreateNew,
    handleSetupPreset,
    handleSave,
    handleDelete,
    confirmDelete,
    handleToggleActive,
  } = useMCPServerManagement(service);

  return (
    <div className="space-y-10">
      <div className="flex justify-between items-center">
        <h2 className="text-xl font-bold tracking-tight">
          {t('mcpServer.title', 'Extensions')}
        </h2>
        <Button onClick={handleCreateNew} className="rounded-xl shadow-md">
          <Plus className="w-4 h-4 mr-2" />
          {t('mcpServer.addServer', 'Add Extension')}
        </Button>
      </div>

      {/* Installed Servers Section (Moved to Top) */}
      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <Package className="w-4 h-4 text-primary" />
          <h3 className="text-sm font-bold uppercase tracking-widest text-muted-foreground font-sans">
            {t('mcpServer.installedExtensions', 'Installed Extensions')}
          </h3>
          <div className="h-px bg-border/50 flex-1 ml-2" />
        </div>

        {isLoading ? (
          <div className="text-center py-12 text-muted-foreground flex flex-col items-center gap-3">
            <LoadingSpinner size="lg" />
            <p className="text-sm font-sans">
              {t('mcpServer.loading', 'Loading extensions...')}
            </p>
          </div>
        ) : servers.length === 0 ? (
          <div className="text-center py-12 text-muted-foreground border border-dashed rounded-[1.5rem] bg-muted/5 flex flex-col items-center gap-2">
            <Package className="w-8 h-8 opacity-20 mb-2" />
            <p className="text-sm font-sans font-medium">
              {t('mcpServer.noServersShort', 'No extensions installed yet.')}
            </p>
            <p className="text-xs font-sans opacity-60">
              {t(
                'mcpServer.installHint',
                'Choose a recommended extension below to get started.',
              )}
            </p>
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
                  isToggling={!!togglingStatus[server.id]}
                />
              ))}
            </div>

            {isValidating && servers.length > 0 && (
              <div className="flex justify-center py-2">
                <span className="text-xs text-muted-foreground animate-pulse font-sans">
                  {t('mcpServer.updating', 'Updating status...')}
                </span>
              </div>
            )}

            {hasNextPage && (
              <div className="flex justify-center pt-4">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={isValidating}
                  onClick={() => setSize((s) => s + 1)}
                  className="rounded-lg font-sans"
                >
                  {isValidating
                    ? t('mcpServer.loadingMore', 'Loading...')
                    : t('mcpServer.loadMore', 'Load more extensions')}
                </Button>
              </div>
            )}
          </>
        )}
      </div>

      <Separator className="opacity-50" />

      {/* Recommended Servers Section (Moved to Bottom) */}
      <div className="animate-in fade-in slide-in-from-bottom-4 duration-700">
        <RecommendedPresets
          presets={presets}
          servers={servers}
          allServers={allServers}
          registryLoaded={registryLoaded}
          onSetupPreset={handleSetupPreset}
        />
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
        onOpenChange={(open) => !open && !isDeleting && setServerToDelete(null)}
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
            <AlertDialogCancel disabled={isDeleting}>
              {t('mcpServer.deleteDialog.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void confirmDelete();
              }}
              disabled={isDeleting}
            >
              {isDeleting && <LoadingSpinner size="sm" className="mr-2" />}
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
