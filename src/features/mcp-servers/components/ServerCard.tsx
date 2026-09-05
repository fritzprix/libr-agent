import React, { useState, useEffect, useCallback } from 'react';
import {
  Server,
  CheckCircle2,
  XCircle,
  Loader2,
  Wrench,
  ShieldCheck,
  ShieldAlert,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
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
import { ServerToolsModal } from './ServerToolsModal';
import { hasOAuthToken, startOAuthFlow, revokeOAuthToken } from '@/lib/backend';
import { listAssistants } from '@/lib/backend/assistants';
import { safeInvoke } from '@/lib/backend/core';
import { toast } from 'sonner';
import {
  humanizeVerificationError,
  pendingVerificationHint,
} from '../utils/verification-feedback';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ServerCard');

interface ServerCardProps {
  server: MCPServerEntity;
  onEdit: (server: MCPServerEntity) => void;
  onDelete: (server: MCPServerEntity) => void;
  onToggleActive: (server: MCPServerEntity, checked: boolean) => void;
  isToggling?: boolean;
  onRevalidate?: () => void;
}

export const ServerCard = React.memo(
  ({
    server,
    onEdit,
    onDelete,
    onToggleActive,
    isToggling,
    onRevalidate,
  }: ServerCardProps) => {
    const { t } = useTranslation('common');
    const navigate = useNavigate();
    const serverName = server.name || t('mcpServer.unnamed', 'Unnamed Server');
    const [isToolsOpen, setIsToolsOpen] = useState(false);
    const verificationStatus = server.verificationStatus;
    const humanizedError = humanizeVerificationError(
      server.lastVerificationError,
    );

    const [hasToken, setHasToken] = useState<boolean>(false);
    const [isAuthorizing, setIsAuthorizing] = useState<boolean>(false);
    const [isRetryingVerify, setIsRetryingVerify] = useState(false);
    const [pendingElapsedSec, setPendingElapsedSec] = useState(0);
    const [isOpeningWizard, setIsOpeningWizard] = useState(false);

    useEffect(() => {
      if (server.authentication?.type === 'oauth2.1') {
        hasOAuthToken(server.id)
          .then(setHasToken)
          .catch(() => setHasToken(false));
      }
    }, [server.id, server.authentication]);

    useEffect(() => {
      if (verificationStatus !== 'pending') {
        setPendingElapsedSec(0);
        return;
      }
      setPendingElapsedSec(0);
      const startedAt = Date.now();
      const timer = window.setInterval(() => {
        setPendingElapsedSec(Math.floor((Date.now() - startedAt) / 1000));
      }, 1000);
      return () => window.clearInterval(timer);
    }, [verificationStatus, server.id]);

    const handleRetryVerify = useCallback(async () => {
      setIsRetryingVerify(true);
      try {
        await safeInvoke('probe_mcp_server', { serverId: server.id });
        toast.success(
          t('mcpServer.toasts.reverifySuccess', 'Connection verified'),
        );
        onRevalidate?.();
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        toast.error(
          t('mcpServer.toasts.reverifyFailed', {
            error: msg,
            defaultValue: 'Verification failed: {{error}}',
          }),
        );
        onRevalidate?.();
      } finally {
        setIsRetryingVerify(false);
      }
    }, [server.id, t, onRevalidate]);

    const handleOpenAppWizard = useCallback(async () => {
      setIsOpeningWizard(true);
      try {
        const assistants = await listAssistants();
        const wizard = assistants.find(
          (assistant) => assistant.name === 'App Wizard',
        );
        if (wizard) {
          navigate(`/agent/draft?assistantId=${encodeURIComponent(wizard.id)}`);
          return;
        }
        navigate('/agent');
        toast.message(
          t(
            'mcpServer.toasts.appWizardFallback',
            'Open App Wizard from Chat and ask it to install Node.js / uv.',
          ),
        );
      } catch (error) {
        logger.error('Failed to open App Wizard', error);
        navigate('/agent');
        toast.error(
          t(
            'mcpServer.toasts.appWizardOpenFailed',
            'Could not open App Wizard. Go to Chat and select App Wizard.',
          ),
        );
      } finally {
        setIsOpeningWizard(false);
      }
    }, [navigate, t]);

    const handleAuthorize = useCallback(async () => {
      if (!server.authentication) return;
      setIsAuthorizing(true);
      try {
        await startOAuthFlow(server.id, server.authentication);
        setHasToken(true);
        toast.success(
          t('mcpServer.toasts.authSuccess', 'Successfully authenticated!'),
        );
        // Probe to trigger re-verification and fetch tools
        await safeInvoke('probe_mcp_server', { serverId: server.id });
        if (onRevalidate) onRevalidate();
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        toast.error(
          t('mcpServer.toasts.authFailed', `Authorization failed: ${msg}`),
        );
      } finally {
        setIsAuthorizing(false);
      }
    }, [server.id, server.authentication, t, onRevalidate]);

    const handleDisconnect = useCallback(async () => {
      try {
        await revokeOAuthToken(server.id);
        setHasToken(false);
        toast.success(
          t('mcpServer.toasts.disconnectSuccess', 'Successfully disconnected.'),
        );
        // Probe to reset status to connection failed/AuthRequired
        await safeInvoke('probe_mcp_server', { serverId: server.id });
        if (onRevalidate) onRevalidate();
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        toast.error(
          t('mcpServer.toasts.disconnectFailed', `Disconnect failed: ${msg}`),
        );
      }
    }, [server.id, t, onRevalidate]);

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
                /* ⚡ Bolt: Added lazy loading and async decoding to prevent eager loading of background images */
                <img
                  src={server.metadata.logo}
                  alt={serverName}
                  loading="lazy"
                  decoding="async"
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
              <div className="mt-1 flex flex-col gap-1">
                {server.authentication?.type === 'oauth2.1' && (
                  <span
                    className={`inline-flex items-center gap-1 text-xs font-semibold ${hasToken ? 'text-blue-600 dark:text-blue-400' : 'text-amber-600 dark:text-amber-500'}`}
                  >
                    {hasToken ? (
                      <>
                        <ShieldCheck className="w-3.5 h-3.5" />
                        {t('mcpServer.authorizedStatus', 'Authorized')}
                      </>
                    ) : (
                      <>
                        <ShieldAlert className="w-3.5 h-3.5 text-amber-500" />
                        {t(
                          'mcpServer.unauthorizedStatus',
                          'Authorization Required',
                        )}
                      </>
                    )}
                  </span>
                )}
                {verificationStatus === 'pending' && (
                  <div className="space-y-1">
                    <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                      <Loader2 className="w-3 h-3 animate-spin" />
                      {t('mcpServer.verifying', 'Verifying...')}
                      {pendingElapsedSec > 0 ? (
                        <span className="tabular-nums text-muted-foreground/80">
                          ({pendingElapsedSec}s)
                        </span>
                      ) : null}
                    </span>
                    <p className="text-xs text-muted-foreground">
                      {pendingVerificationHint(pendingElapsedSec)}
                    </p>
                  </div>
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
                  <div className="space-y-1">
                    <span className="inline-flex items-center gap-1 text-xs text-destructive font-medium">
                      <XCircle className="w-3.5 h-3.5" />
                      {t('mcpServer.verificationFailed', 'Connection failed')}
                    </span>
                    {humanizedError ? (
                      <>
                        <p
                          className="text-xs text-destructive/90 break-words"
                          title={humanizedError.raw}
                        >
                          {humanizedError.summary}
                        </p>
                        {humanizedError.guidance ? (
                          <p className="text-xs text-muted-foreground break-words">
                            {humanizedError.guidance}
                          </p>
                        ) : null}
                        {humanizedError.runtime ? (
                          <Button
                            type="button"
                            variant="link"
                            size="sm"
                            className="h-auto p-0 text-xs"
                            disabled={isOpeningWizard}
                            onClick={() => void handleOpenAppWizard()}
                          >
                            {isOpeningWizard ? (
                              <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                            ) : null}
                            {t('mcpServer.openAppWizard', 'Open App Wizard')}
                          </Button>
                        ) : null}
                      </>
                    ) : server.lastVerificationError ? (
                      <p
                        className="text-xs text-destructive/90 break-words"
                        title={server.lastVerificationError}
                      >
                        {server.lastVerificationError}
                      </p>
                    ) : null}
                  </div>
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

              {verificationStatus === 'error' && (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={isRetryingVerify}
                  onClick={() => void handleRetryVerify()}
                  aria-label={t('mcpServer.retryVerify', {
                    name: serverName,
                    defaultValue: 'Retry verification for {{name}}',
                  })}
                >
                  {isRetryingVerify ? (
                    <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                  ) : null}
                  {t('mcpServer.retryVerifyShort', 'Retry')}
                </Button>
              )}

              {server.authentication?.type === 'oauth2.1' &&
                (hasToken ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleDisconnect}
                    className="text-amber-600 hover:text-amber-700 border-amber-200 hover:bg-amber-50 dark:hover:bg-amber-950/20"
                  >
                    {t('mcpServer.disconnect', 'Disconnect')}
                  </Button>
                ) : (
                  <Button
                    variant="default"
                    size="sm"
                    disabled={isAuthorizing}
                    onClick={handleAuthorize}
                    className="bg-blue-600 hover:bg-blue-700 text-white font-semibold"
                  >
                    {isAuthorizing && (
                      <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                    )}
                    {t('mcpServer.authorize', 'Authorize')}
                  </Button>
                ))}
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
      prev.server.verificationStatus === next.server.verificationStatus &&
      prev.server.lastVerificationError === next.server.lastVerificationError &&
      prev.server.updatedAt?.getTime() === next.server.updatedAt?.getTime() &&
      prev.isToggling === next.isToggling &&
      prev.onEdit === next.onEdit &&
      prev.onDelete === next.onDelete &&
      prev.onToggleActive === next.onToggleActive &&
      prev.onRevalidate === next.onRevalidate
    );
  },
);

ServerCard.displayName = 'ServerCard';
