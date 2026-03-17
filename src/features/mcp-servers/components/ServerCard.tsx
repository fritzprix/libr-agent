import React, { useState } from 'react';
import { Server, CheckCircle2, XCircle, Loader2, Wrench } from 'lucide-react';
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
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import type { VerificationStatus } from '../hooks/useMCPServerManagement';
import { ServerToolsModal } from './ServerToolsModal';

interface ServerCardProps {
  server: MCPServerEntity;
  onEdit: (server: MCPServerEntity) => void;
  onDelete: (server: MCPServerEntity) => void;
  onToggleActive: (server: MCPServerEntity, checked: boolean) => void;
  verificationStatus?: VerificationStatus;
  isToggling?: boolean;
}

export const ServerCard = React.memo(
  ({
    server,
    onEdit,
    onDelete,
    onToggleActive,
    verificationStatus,
    isToggling,
  }: ServerCardProps) => {
    const { t } = useTranslation('common');
    const serverName = server.name || t('mcpServer.unnamed', 'Unnamed Server');
    const [isToolsOpen, setIsToolsOpen] = useState(false);

    return (
      <Card className="relative overflow-hidden">
        {/* Verification progress bar - animating shimmer when pending */}
        {verificationStatus === 'pending' && (
          <div className="absolute top-0 left-0 right-0 h-0.5 overflow-hidden">
            <div className="h-full w-1/2 bg-primary/60 rounded-full animate-[slide_1.2s_ease-in-out_infinite]" />
          </div>
        )}
        {verificationStatus === 'success' && (
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-emerald-500/70" />
        )}
        {verificationStatus === 'error' && (
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-destructive/70" />
        )}
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div className="flex gap-3 items-start flex-1">
            {/* Server logo */}
            <div className="w-8 h-8 rounded-md overflow-hidden flex-shrink-0 mt-0.5 border border-border/50">
              {server.metadata?.logo ? (
                <img
                  src={server.metadata.logo}
                  alt={serverName}
                  className="w-full h-full object-contain"
                  onError={(e) => {
                    (e.currentTarget as HTMLImageElement).style.display =
                      'none';
                    (
                      e.currentTarget.nextElementSibling as HTMLElement | null
                    )?.classList.remove('hidden');
                  }}
                />
              ) : null}
              <div
                className={`w-full h-full bg-muted flex items-center justify-center ${server.metadata?.logo ? 'hidden' : ''}`}
              >
                <Server className="w-4 h-4 text-muted-foreground" />
              </div>
            </div>
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
                  ` • ${(() => {
                    const urlString = (server.transport as { url: string }).url;
                    try {
                      const urlObj = new URL(urlString);
                      // server.metadata should be queried strictly when parsing structure
                      const varDefs = (
                        server.metadata as {
                          variableDefinitions?: Record<
                            string,
                            { target?: string }
                          >;
                        }
                      )?.variableDefinitions;
                      if (varDefs) {
                        Object.entries(varDefs).forEach(([key, def]) => {
                          if (
                            def.target === 'url-param' &&
                            urlObj.searchParams.has(key)
                          ) {
                            urlObj.searchParams.delete(key);
                          }
                        });
                      }
                      return urlObj.toString();
                    } catch {
                      return urlString; // Fallback to raw string if invalid URL
                    }
                  })()}`}
              </p>
              {/* Tool count / verification status — mutually exclusive display */}
              <div className="mt-1">
                {verificationStatus === 'pending' && (
                  <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                    <Loader2 className="w-3 h-3 animate-spin" />
                    {t('mcpServer.verifying', 'Verifying...')}
                  </span>
                )}
                {verificationStatus === 'success' && (
                  <span className="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400 font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" />
                    {server.toolCount !== undefined && server.toolCount !== null
                      ? t('mcpServer.verifiedWithCount', {
                          count: server.toolCount,
                          defaultValue: '{{count}} tools verified',
                        })
                      : t('mcpServer.verified', 'Verified')}
                  </span>
                )}
                {verificationStatus === 'error' && (
                  <span className="inline-flex items-center gap-1 text-xs text-destructive font-medium">
                    <XCircle className="w-3.5 h-3.5" />
                    {t('mcpServer.verificationFailed', 'Connection failed')}
                  </span>
                )}
                {/* Only show "not verified" when no active verification and no tool count */}
                {!verificationStatus &&
                  server.toolCount !== undefined &&
                  server.toolCount !== null && (
                    <p className="text-xs text-muted-foreground">
                      {t('mcpServer.toolsAvailable', {
                        count: server.toolCount,
                        defaultValue: '{{count}} tools',
                      })}
                    </p>
                  )}
                {!verificationStatus &&
                  (server.toolCount === undefined ||
                    server.toolCount === null) && (
                    <p className="text-xs text-muted-foreground italic">
                      {t(
                        'mcpServer.toolCountUnknown',
                        'Tool count unknown (not yet verified)',
                      )}
                    </p>
                  )}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex flex-col items-end gap-1">
              <div className="flex items-center gap-1">
                {isToggling && <LoadingSpinner size="sm" />}
                <span className="text-xs text-muted-foreground">
                  {t('mcpServer.active', 'Active')}
                </span>
              </div>
              <Switch
                checked={server.isActive}
                onCheckedChange={(checked) => onToggleActive(server, checked)}
                disabled={isToggling}
                aria-label={t('mcpServer.toggleActive', {
                  name: serverName,
                  defaultValue: 'Toggle active state for {{name}}',
                })}
              />
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="flex justify-between items-center w-full">
            <div className="flex gap-2">
              {server.toolCount !== undefined &&
                server.toolCount !== null &&
                server.toolCount > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setIsToolsOpen(true)}
                    aria-label={t('mcpServer.browseTools', {
                      name: serverName,
                      defaultValue: 'Browse tools for {{name}}',
                    })}
                  >
                    <Wrench className="h-3 w-3 mr-1" />
                    {t('mcpServer.tools', 'Tools')}
                  </Button>
                )}
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
            </div>
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
        <ServerToolsModal
          serverId={server.id}
          serverName={serverName}
          isOpen={isToolsOpen}
          onClose={() => setIsToolsOpen(false)}
        />
      </Card>
    );
  },
  (prev, next) => {
    return (
      prev.server.id === next.server.id &&
      prev.server.name === next.server.name &&
      prev.server.isActive === next.server.isActive &&
      prev.server.toolCount === next.server.toolCount &&
      prev.server.updatedAt?.getTime() === next.server.updatedAt?.getTime() &&
      prev.verificationStatus === next.verificationStatus &&
      prev.isToggling === next.isToggling &&
      prev.onEdit === next.onEdit &&
      prev.onDelete === next.onDelete &&
      prev.onToggleActive === next.onToggleActive
    );
  },
);

ServerCard.displayName = 'ServerCard';
