import { useState, useCallback, useMemo } from 'react';
import useSWRInfinite from 'swr/infinite';
import useSWRImmutable from 'swr/immutable';
import { useTranslation } from 'react-i18next';
import { createId } from '@paralleldrive/cuid2';
import { toast } from 'sonner';
import { safeInvoke as invoke } from '@/lib/backend/core';
import { MCPServerEntity } from '@/models/chat';
import { McpServerService } from '@/lib/services/mcp-server-service';
import {
  listMCPServerPresets,
  type MCPServerPreset,
} from '@/lib/backend/mcp-server-config';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { useSettings } from '@/hooks/use-settings';
import { getLogger } from '@/lib/logger';
import { sanitizePresetEnv, buildPresetMetadata } from '../utils/preset-utils';
import type { MCPTool } from '@/lib/mcp/protocol/tool';

export type VerificationStatus = 'pending' | 'success' | 'error';

const logger = getLogger('MCPServerManagement');

export function useMCPServerManagement(service?: McpServerService) {
  const { t } = useTranslation('common');
  const { saveServer, deleteServer, toggleActive } = useMCPServerRegistry();
  const { value: settings } = useSettings();

  // Verification status per server id: 'pending' | 'success' | 'error'
  const [verificationStatus, setVerificationStatus] = useState<
    Record<string, VerificationStatus>
  >({});

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
  const [isDeleting, setIsDeleting] = useState(false);
  const [togglingStatus, setTogglingStatus] = useState<Record<string, boolean>>(
    {},
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

  const handleSetupPreset = useCallback((preset: MCPServerPreset) => {
    const transport: MCPServerEntity['transport'] =
      preset.transportType === 'sse' && preset.url
        ? {
            type: 'http-sse',
            url: preset.url,
            enableSSE: true,
            headers: sanitizePresetEnv(preset.env),
          }
        : {
            type: 'stdio',
            command: preset.command || 'uvx',
            args: preset.args || [],
            env: sanitizePresetEnv(preset.env),
          };

    const newServer: MCPServerEntity = {
      id: createId(),
      name: preset.name,
      isActive: true,
      createdAt: new Date(),
      updatedAt: new Date(),
      metadata: buildPresetMetadata(preset),
      transport,
    };
    setEditingServer(newServer);
  }, []);

  const handleSave = useCallback(
    async (server: MCPServerEntity) => {
      try {
        const saved = await saveServer({
          ...server,
          createdAt: server.createdAt ?? new Date(),
          updatedAt: new Date(),
        });
        await mutateServers();
        setEditingServer(null);
        toast.success(
          t('mcpServer.toasts.saved', 'Extension saved successfully'),
        );

        // Background dry-run: probe using the DB-assigned ID from the saved entity.
        // For new servers, server.id is a temporary createId(); saved.id is the real DB ID.
        setVerificationStatus((prev) => ({ ...prev, [saved.id]: 'pending' }));
        try {
          // Spawns the server process, fetches tools, tears down — all in one Rust call.
          // tool_count is also persisted to DB automatically inside the command.
          const tools = await invoke<MCPTool[]>('probe_mcp_server', {
            serverId: saved.id,
          });

          if (tools.length === 0) {
            throw new Error(
              `No tools returned from server "${saved.name}" — connection may have failed silently`,
            );
          }

          await mutateServers();
          setVerificationStatus((prev) => ({
            ...prev,
            [saved.id]: 'success',
          }));
          logger.info(`Verified "${saved.name}": ${tools.length} tool(s)`);
        } catch (verifyErr) {
          setVerificationStatus((prev) => ({ ...prev, [saved.id]: 'error' }));
          logger.warn(`Verification failed for "${saved.name}"`, verifyErr);
        }
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
      }
    },
    [saveServer, mutateServers, t],
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
    verificationStatus,
    handleCreateNew,
    handleSetupPreset,
    handleSave,
    handleDelete,
    confirmDelete,
    handleToggleActive,
  };
}
