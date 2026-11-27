import React, { useState, useEffect, useCallback, useRef } from 'react';
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
  Badge,
} from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { ServerToolsModal } from './ServerToolsModal';
import {
  cancelOAuthFlow,
  getOAuthFlowLastOutcome,
  hasOAuthToken,
  isOAuthFlowActive,
  revokeOAuthToken,
  startOAuthFlow,
} from '@/lib/backend';
import { listAssistants } from '@/lib/backend/assistants';
import { safeInvoke } from '@/lib/backend/core';
import { toast } from 'sonner';
import {
  humanizeVerificationError,
  pendingVerificationHint,
} from '../utils/verification-feedback';
import {
  applyBackendFlowOutcome,
  beginAuthFlow,
  ensureOAuthFlowEndedListener,
  getAuthFlow,
  subscribeToAuthFlow,
  type AuthFlowRecord,
} from '../authFlowStore';
import { getTransportEndpointLabel } from '../utils/transport-display';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ServerCard');

/**
 * Machine-readable prefixes emitted by the backend OAuth flow (#3040 G1.6).
 * The card classifies the outcome by these prefixes, never by matching
 * human-readable text: a provider denial whose description happens to read
 * "The user cancelled the request" must stay a provider error.
 */
const AUTH_ERROR_PREFIX_CANCELLED = 'AUTH_CANCELLED:';
const AUTH_ERROR_PREFIX_TIMEOUT = 'AUTH_TIMEOUT:';

interface AuthNotice {
  kind: 'cancelled' | 'timedOut' | 'providerError';
  /** Human-readable cause shown on the card (#3040 G1.6). */
  message: string;
}

function classifyAuthFailure(message: string): AuthNotice {
  if (message.startsWith(AUTH_ERROR_PREFIX_CANCELLED)) {
    return { kind: 'cancelled', message: stripAuthPrefix(message) };
  }
  if (message.startsWith(AUTH_ERROR_PREFIX_TIMEOUT)) {
    return { kind: 'timedOut', message: stripAuthPrefix(message) };
  }
  return { kind: 'providerError', message };
}

function stripAuthPrefix(message: string): string {
  const separator = message.indexOf(':');
  return separator === -1 ? message : message.slice(separator + 1).trim();
}

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
    const [authNotice, setAuthNotice] = useState<AuthNotice | null>(null);
    // Identity of the flow record whose outcome has already been applied to
    // this card, so a settled outcome is never applied twice.
    const appliedRecordRef = useRef<AuthFlowRecord | null>(null);
    // True only while this card is waiting on a backend-held flow (#3040 G4.1).
    // Cold remounts must not replay a stored success/error.
    const waitingForBackendFlowRef = useRef(false);
    const onRevalidateRef = useRef(onRevalidate);
    onRevalidateRef.current = onRevalidate;
    const [isRetryingVerify, setIsRetryingVerify] = useState(false);
    const [pendingElapsedSec, setPendingElapsedSec] = useState(0);
    const [isOpeningWizard, setIsOpeningWizard] = useState(false);

    useEffect(() => {
      if (server.authentication?.type !== 'oauth2.1') return;
      // Cards without a local promise learn endings from the backend emit
      // (and a takeable last outcome only while waiting) (#3040 G3.1 / G4.1).
      ensureOAuthFlowEndedListener();
      hasOAuthToken(server.id)
        .then(setHasToken)
        .catch(() => setHasToken(false));

      // Sync from the flow store first (flows started in this app session,
      // including ones begun by a card that has since unmounted, #3040 G2.2),
      // then fall back to the backend's single-flight slot (flows still
      // running across an app reload, #3040 G1.5).
      const applySettledOutcome = (record: AuthFlowRecord) => {
        if (appliedRecordRef.current === record || !record.settled) return;
        appliedRecordRef.current = record;
        waitingForBackendFlowRef.current = false;
        if (record.settled.kind === 'success') {
          setAuthNotice(null);
          // Connected state always comes from the keychain (#3040 G4.1).
          void hasOAuthToken(server.id)
            .then(setHasToken)
            .catch(() => setHasToken(false));
          // Ownerless success must still probe / revalidate (#3040 G4.2).
          void safeInvoke('probe_mcp_server', { serverId: server.id })
            .then(() => onRevalidateRef.current?.())
            .catch(() => undefined);
        } else {
          setAuthNotice(classifyAuthFailure(record.settled.message));
        }
      };

      const sync = () => {
        const record = getAuthFlow(server.id);
        if (record && record.settled === null) {
          setIsAuthorizing(true);
          return;
        }
        if (record && record.settled) {
          applySettledOutcome(record);
          setIsAuthorizing(false);
          return;
        }
        isOAuthFlowActive(server.id)
          .then(async (backendActive) => {
            // A flow may have started in the store while this check was in
            // flight; never override it with a stale answer.
            if (getAuthFlow(server.id) !== null) return;
            if (backendActive) {
              waitingForBackendFlowRef.current = true;
              setIsAuthorizing(true);
              return;
            }
            setIsAuthorizing(false);
            // Only a card that was waiting may take the last outcome (#3040 G4.1).
            if (!waitingForBackendFlowRef.current) return;
            try {
              const last = await getOAuthFlowLastOutcome(server.id);
              if (getAuthFlow(server.id) !== null) return;
              if (!last) {
                waitingForBackendFlowRef.current = false;
                return;
              }
              applyBackendFlowOutcome(last);
            } catch {
              // Backend unreachable: keep waiting until the emit arrives.
            }
          })
          .catch(() => setIsAuthorizing(false));
      };

      waitingForBackendFlowRef.current = false;
      const unsubscribe = subscribeToAuthFlow(server.id, sync);
      sync();
      return unsubscribe;
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

    const authNoticeText = useCallback(
      (notice: AuthNotice): string => {
        switch (notice.kind) {
          case 'cancelled':
            return t(
              'mcpServer.toasts.authCancelled',
              'Authorization cancelled.',
            );
          case 'timedOut':
            return t(
              'mcpServer.toasts.authTimedOut',
              'Authorization timed out. Try Authorize again.',
            );
          default:
            return t('mcpServer.toasts.authFailed', {
              error: notice.message,
              defaultValue: 'Authorization failed: {{error}}',
            });
        }
      },
      [t],
    );

    const handleAuthorize = useCallback(async () => {
      if (!server.authentication) return;
      const authentication = server.authentication;
      setAuthNotice(null);
      const { record, started } = beginAuthFlow(server.id, async () => {
        try {
          await startOAuthFlow(server.id, authentication);
          return { kind: 'success' as const };
        } catch (error) {
          return {
            kind: 'failure' as const,
            message: error instanceof Error ? error.message : String(error),
          };
        }
      });
      setIsAuthorizing(true);
      const outcome = await record.promise;
      if (!started) return;
      // Only the initiating card reports toasts and probes (#3040 G2.2).
      if (outcome.kind === 'success') {
        toast.success(
          t('mcpServer.toasts.authSuccess', 'Successfully authenticated!'),
        );
        // Probe to trigger re-verification and fetch tools
        await safeInvoke('probe_mcp_server', { serverId: server.id });
        onRevalidate?.();
      } else {
        const notice = classifyAuthFailure(outcome.message);
        const label = authNoticeText(notice);
        if (notice.kind === 'cancelled') toast.message(label);
        else toast.error(label);
      }
    }, [server.id, server.authentication, t, onRevalidate, authNoticeText]);

    const handleCancelAuthorize = useCallback(async () => {
      try {
        await cancelOAuthFlow(server.id);
        if (getAuthFlow(server.id) === null) {
          // A card remounted onto a backend-held flow has no local promise to
          // resolve after cancel — sync the card itself (#3040 G1.5).
          setIsAuthorizing(false);
          setAuthNotice({
            kind: 'cancelled',
            message: t(
              'mcpServer.toasts.authCancelled',
              'Authorization cancelled.',
            ),
          });
        }
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        toast.error(
          t('mcpServer.toasts.authCancelFailed', {
            error: msg,
            defaultValue: 'Could not cancel: {{error}}',
          }),
        );
        // Refused / failed cancel: this card is waiting. Take the terminal
        // outcome if the flow already ended (#3040 G3.1 / G4.1).
        waitingForBackendFlowRef.current = true;
        try {
          const last = await getOAuthFlowLastOutcome(server.id);
          if (last && getAuthFlow(server.id) === null) {
            applyBackendFlowOutcome(last);
          }
        } catch {
          // Keep spinner until the emit arrives.
        }
      }
    }, [server.id, t]);

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

    const transportEndpoint = getTransportEndpointLabel(
      server.transport,
      server.metadata,
    );

    return (
      <Card className="relative flex h-full flex-col overflow-hidden">
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
        <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 pb-2">
          <div className="flex min-w-0 flex-1 items-start gap-3">
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
            <div className="min-w-0 flex-1">
              <CardTitle className="text-base break-words">
                {serverName}
                {server.pluginSource ? (
                  <Badge
                    variant="secondary"
                    className="ml-2 align-middle"
                    title={t(
                      'mcpServer.pluginOwnedHint',
                      'Installed by a plugin. Edit and delete are disabled; uninstall the plugin to remove it.',
                    )}
                  >
                    {t('mcpServer.pluginOwned', 'Plugin')}
                  </Badge>
                ) : null}
              </CardTitle>
              <p className="text-sm text-muted-foreground mt-1 break-words">
                {server.metadata?.description ||
                  t('mcpServer.noDescription', 'No description')}
              </p>
              <p className="text-xs text-muted-foreground mt-1 break-all">
                {t('mcpServer.transport', 'Transport')}: {server.transport.type}
                {transportEndpoint ? ` • ${transportEndpoint}` : ''}
              </p>
              {/* Tool count / verification status — mutually exclusive display */}
              <div className="mt-1 flex flex-col gap-1">
                {server.authentication?.type === 'oauth2.1' && (
                  <div className="flex items-center gap-2">
                    {/* No flex-wrap: the status text truncates so the
                        Authorize / Disconnect button never wraps or clips
                        at narrow card widths (#3000 G1.1/G1.2). */}
                    <span
                      className={`inline-flex min-w-0 items-center gap-1 text-xs font-semibold ${hasToken ? 'text-blue-600 dark:text-blue-400' : 'text-amber-600 dark:text-amber-500'}`}
                    >
                      {hasToken ? (
                        <>
                          <ShieldCheck className="w-3.5 h-3.5 shrink-0" />
                          <span
                            className="truncate"
                            title={t(
                              'mcpServer.authorizedStatus',
                              'Authorized',
                            )}
                          >
                            {t('mcpServer.authorizedStatus', 'Authorized')}
                          </span>
                        </>
                      ) : (
                        <>
                          <ShieldAlert className="w-3.5 h-3.5 shrink-0 text-amber-500" />
                          <span
                            className="truncate"
                            title={t(
                              'mcpServer.unauthorizedStatus',
                              'Authorization Required',
                            )}
                          >
                            {t(
                              'mcpServer.unauthorizedStatus',
                              'Authorization Required',
                            )}
                          </span>
                        </>
                      )}
                    </span>
                    {hasToken ? (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleDisconnect}
                        data-testid="oauth-disconnect"
                        className="h-6 shrink-0 px-2 text-xs text-amber-600 hover:text-amber-700 border-amber-200 hover:bg-amber-50 dark:hover:bg-amber-950/20"
                      >
                        {t('mcpServer.disconnect', 'Disconnect')}
                      </Button>
                    ) : isAuthorizing ? (
                      /* The wait is cancellable end-to-end (#3040): the
                         button flips to Cancel while the flow runs. */
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleCancelAuthorize}
                        data-testid="oauth-cancel"
                        className="h-6 shrink-0 px-2 text-xs"
                      >
                        <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                        {t('mcpServer.cancelAuthorize', 'Cancel')}
                      </Button>
                    ) : (
                      <Button
                        variant="default"
                        size="sm"
                        onClick={handleAuthorize}
                        data-testid="oauth-authorize"
                        className="h-6 shrink-0 px-2 text-xs bg-blue-600 hover:bg-blue-700 text-white font-semibold"
                      >
                        {t('mcpServer.authorize', 'Authorize')}
                      </Button>
                    )}
                  </div>
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
                    {/* Retry belongs next to the failed status line, not in the
                        action row, so every card keeps the same Tools / Edit /
                        Delete slots (#3000 G3.2). */}
                    {/* Same rule as the auth row: text truncates, Retry
                        stays fully visible (#3000 G1.3). */}
                    <div className="flex items-center gap-2">
                      <span className="inline-flex min-w-0 items-center gap-1 text-xs text-destructive font-medium">
                        <XCircle className="w-3.5 h-3.5 shrink-0" />
                        <span
                          className="truncate"
                          title={t(
                            'mcpServer.verificationFailed',
                            'Connection failed',
                          )}
                        >
                          {t(
                            'mcpServer.verificationFailed',
                            'Connection failed',
                          )}
                        </span>
                      </span>
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={isRetryingVerify}
                        onClick={() => void handleRetryVerify()}
                        className="h-6 shrink-0 px-2 text-xs"
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
                    </div>
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
          <div className="flex shrink-0 items-center gap-2">
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
        <CardContent className="mt-auto">
          {authNotice && (
            <div
              data-testid="oauth-result"
              role="status"
              className={
                authNotice.kind === 'cancelled'
                  ? 'mb-3 rounded-md border border-neutral-300 bg-neutral-50 px-3 py-2 text-xs text-neutral-700 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-300'
                  : 'mb-3 rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-700 dark:bg-amber-950/30 dark:text-amber-200'
              }
            >
              {authNoticeText(authNotice)}
            </div>
          )}
          <div className="flex w-full items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              {/* Every card shows Tools / Edit / Delete in the same slots
                  (#3000 G3.1); Tools is disabled when there are no tools. */}
              <Button
                variant="outline"
                size="sm"
                disabled={!server.toolCount}
                title={
                  server.toolCount
                    ? undefined
                    : t(
                        'mcpServer.toolCountUnknown',
                        'Tool count unknown (not yet verified)',
                      )
                }
                onClick={() => setIsToolsOpen(true)}
                aria-label={t('mcpServer.browseTools', {
                  name: serverName,
                  defaultValue: 'Browse tools for {{name}}',
                })}
              >
                <Wrench className="h-3 w-3 mr-1" />
                {t('mcpServer.tools', 'Tools')}
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={Boolean(server.pluginSource)}
                onClick={() => onEdit(server)}
                title={
                  server.pluginSource
                    ? t(
                        'mcpServer.pluginOwnedHint',
                        'Installed by a plugin. Edit and delete are disabled; uninstall the plugin to remove it.',
                      )
                    : undefined
                }
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
              className="shrink-0"
              disabled={Boolean(server.pluginSource)}
              onClick={() => onDelete(server)}
              title={
                server.pluginSource
                  ? t(
                      'mcpServer.pluginOwnedHint',
                      'Installed by a plugin. Edit and delete are disabled; uninstall the plugin to remove it.',
                    )
                  : undefined
              }
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
