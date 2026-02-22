import React from 'react';
import { useTranslation } from 'react-i18next';
import { MCPServerEntity } from '@/models/chat';
import {
  Button,
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from '@/components/ui';
import { Switch } from '@/components/ui/switch';

interface ServerCardProps {
  server: MCPServerEntity;
  onEdit: (server: MCPServerEntity) => void;
  onDelete: (server: MCPServerEntity) => void;
  onToggleActive: (server: MCPServerEntity, checked: boolean) => void;
}

export const ServerCard = React.memo(
  ({ server, onEdit, onDelete, onToggleActive }: ServerCardProps) => {
    const { t } = useTranslation('common');
    const serverName = server.name || t('mcpServer.unnamed', 'Unnamed Server');

    return (
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div className="flex-1">
            <CardTitle className="text-base">{serverName}</CardTitle>
            <p className="text-sm text-muted-foreground mt-1">
              {server.metadata?.description ||
                t('mcpServer.noDescription', 'No description')}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {t('mcpServer.transport', 'Transport')}: {server.transport.type}
              {server.transport.type === 'stdio' &&
                ` • ${server.transport.command}`}
              {((server.transport.type as string) === 'http' ||
                server.transport.type === 'http-sse') &&
                ` • ${(server.transport as { url: string }).url}`}
            </p>
            {server.toolCount !== undefined && server.toolCount !== null && (
              <p className="text-xs text-muted-foreground mt-1">
                {t('mcpServer.toolsAvailable', {
                  count: server.toolCount,
                  defaultValue: '{{count}} tool available',
                })}
              </p>
            )}
            {(server.toolCount === undefined || server.toolCount === null) && (
              <p className="text-xs text-muted-foreground italic mt-1">
                {t(
                  'mcpServer.toolCountUnknown',
                  'Tool count unknown (not yet verified)',
                )}
              </p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <div className="flex flex-col items-end gap-1">
              <span className="text-xs text-muted-foreground">
                {t('mcpServer.active', 'Active')}
              </span>
              <Switch
                checked={server.isActive}
                onCheckedChange={(checked) => onToggleActive(server, checked)}
                aria-label={t('mcpServer.toggleActive', {
                  name: serverName,
                  defaultValue: 'Toggle active state for {{name}}',
                })}
              />
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => onEdit(server)}
              aria-label={t('mcpServer.editServer', {
                name: serverName,
                defaultValue: 'Edit {{name}}',
              })}
            >
              {t('mcpServer.edit', 'Edit')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => onDelete(server)}
              aria-label={t('mcpServer.deleteServer', {
                name: serverName,
                defaultValue: 'Delete {{name}}',
              })}
            >
              {t('mcpServer.delete', 'Delete')}
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
