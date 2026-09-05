import { useState, useCallback, useMemo, useEffect } from 'react';
import useSWRInfinite from 'swr/infinite';
import useSWRImmutable from 'swr/immutable';
import { useTranslation } from 'react-i18next';
import { createId } from '@paralleldrive/cuid2';
import { toast } from 'sonner';
import { MCPServerEntity } from '@/models/chat';
import { McpServerService } from '@/lib/services/mcp-server-service';
import {
  listMCPServerPresets,
  type MCPServerPreset,
} from '@/lib/backend/mcp-server-config';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { useBackendResource } from '@/context/GlobalEventContext';
import { useSettings } from '@/hooks/use-settings';
import { getLogger } from '@/lib/logger';
import {
  buildServerEntityFromPreset,
  presetNeedsUserConfig,
} from '../utils/preset-utils';

const logger = getLogger('MCPServerManagement');

export function useMCPServerManagement(service?: McpServerService) {
  const { t } = useTranslation('common');
  const {
    allServers,
    loaded: registryLoaded,
    error: registryError,
    saveServer,
    deleteServer,
    toggleActive,
    ensureLoaded,
    refreshAll,
  } = useMCPServerRegistry();
  const { value: settings } = useSettings();

  useEffect(() => {
    void ensureLoaded();
  }, [ensureLoaded]);

  // Fetch Recommended Presets
  const { data: presets } = useSWRImmutable<MCPServerPreset[]>(
    'mcpServerPresets',
    listMCPServerPresets,
  );

  const mcpServerService = useMemo(() => {
    return service || new McpServerService(settings.agentHubUrl);
  }, [settings.agentHubUrl, service]);

  // Follow SessionContext pattern: useSWRInfinite + Page<T>
  const {
    data,
    isLoading,
    isValidating,
    setSize,
    mutate: mutateServers,
  } = useSWRInfinite(
    (pageIndex) => ['mcpServers', settings.agentHubUrl, pageIndex],
    async ([, , pageIndex]) => {
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
  const [isDeleting, setIsDeleting] = useState(false);
  const [togglingStatus, setTogglingStatus] = useState<Record<string, boolean>>(
    {},
  );
  /** Preset names currently in one-click install (before they appear as Installed). */
  const [installingPresetNames, setInstallingPresetNames] = useState<
    ReadonlySet<string>
  >(() => new Set());

  useBackendResource('mcpServer', () => {
    void mutateServers();
  });

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

  const handleSetupPreset = useCallback((preset: MCPServerPreset) => {
    setEditingServer(buildServerEntityFromPreset(preset, createId()));
  }, []);

  const handleSave = useCallback(
    async (server: MCPServerEntity) => {
      const isUpdate = allServers.some(
        (installed) => installed.id === server.id,
      );
      const isRegistryInstall =
        !isUpdate && server.metadata?.source === 'registry';

      try {
        const saved = await saveServer({
          ...server,
          createdAt: server.createdAt ?? new Date(),
          updatedAt: new Date(),
        });

        await mutateServers();
        setEditingServer(null);

        const needsBackgroundVerify =
          !isUpdate || saved.verificationStatus === 'pending';

        if (needsBackgroundVerify) {
          toast.success(
            isRegistryInstall
              ? t(
                  'mcpServer.toasts.installedVerifying',
                  'Extension installed. Verifying connection in the background…',
                )
              : t(
                  'mcpServer.toasts.savedVerifying',
                  'Extension saved. Verifying connection in the background…',
                ),
          );
        } else {
          toast.success(
            isRegistryInstall
              ? t(
                  'mcpServer.toasts.installed',
                  'Extension installed successfully',
                )
              : t('mcpServer.toasts.saved', 'Extension saved successfully'),
          );
        }

        return saved;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Unknown error';
        toast.error(
          t('mcpServer.toasts.saveFailed', {
            error: message,
            defaultValue: 'Failed to save extension: {{error}}',
          }),
        );
        logger.error('Failed to save extension', error);
        throw error;
      }
    },
    [saveServer, mutateServers, t, allServers],
  );

  /**
   * Recommended card click: zero-config presets install immediately;
   * presets that need keys/OAuth still open the dialog.
   */
  const handleInstallOrConfigurePreset = useCallback(
    async (preset: MCPServerPreset) => {
      if (presetNeedsUserConfig(preset)) {
        handleSetupPreset(preset);
        return;
      }

      setInstallingPresetNames((prev) => {
        const next = new Set(prev);
        next.add(preset.name);
        return next;
      });

      try {
        await handleSave(buildServerEntityFromPreset(preset, createId()));
      } catch {
        // handleSave already toasted
      } finally {
        setInstallingPresetNames((prev) => {
          const next = new Set(prev);
          next.delete(preset.name);
          return next;
        });
      }
    },
    [handleSave, handleSetupPreset],
  );

  const handleDelete = useCallback((server: MCPServerEntity) => {
    setServerToDelete(server);
  }, []);

  const confirmDelete = useCallback(async () => {
    if (!serverToDelete || isDeleting) return;

    setIsDeleting(true);
    try {
      await deleteServer(serverToDelete.id);
      await mutateServers();
      toast.success(
        t('mcpServer.toasts.deleted', 'Extension deleted successfully'),
      );
      setServerToDelete(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(
        t('mcpServer.toasts.deleteFailed', {
          error: message,
          defaultValue: 'Failed to delete extension: {{error}}',
        }),
      );
      logger.error('Failed to delete MCP server', error);
    } finally {
      setIsDeleting(false);
    }
  }, [serverToDelete, deleteServer, mutateServers, t]);

  const handleToggleActive = useCallback(
    async (server: MCPServerEntity, checked: boolean) => {
      setTogglingStatus((prev) => ({ ...prev, [server.id]: true }));
      try {
        await toggleActive(server.id, checked);
        await mutateServers();
        toast.success(
          t('mcpServer.toasts.toggled', {
            status: checked
              ? t('mcpServer.toasts.activated', 'activated')
              : t('mcpServer.toasts.deactivated', 'deactivated'),
            defaultValue: 'Extension {{status}}',
          }),
        );
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Unknown error';
        toast.error(
          t('mcpServer.toasts.toggleFailed', {
            error: message,
            defaultValue: 'Failed to toggle extension: {{error}}',
          }),
        );
        logger.error('Failed to toggle MCP server active status', error);
      } finally {
        setTogglingStatus((prev) => {
          const next = { ...prev };
          delete next[server.id];
          return next;
        });
      }
    },
    [toggleActive, mutateServers, t],
  );

  return {
    servers,
    allServers,
    registryLoaded,
    registryError,
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
    handleInstallOrConfigurePreset,
    installingPresetNames,
    retryRegistryLoad: refreshAll,
    handleSave,
    handleDelete,
    confirmDelete,
    handleToggleActive,
    mutateServers,
  };
}
