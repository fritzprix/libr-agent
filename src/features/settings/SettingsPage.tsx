import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useNavigate } from 'react-router-dom';
import { BrainCircuit } from 'lucide-react';
import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '@/hooks/use-settings';
import { useTranslation } from 'react-i18next';
import i18n from '@/lib/i18n';
import type { ServiceConfig } from '@/context/SettingsContext';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { TerminalModelPicker } from '@/features/chat/ModelPicker';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogFooter,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogAction,
  AlertDialogCancel,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';
import { MCPServerManagement } from './MCPServerManagement';
import { getLogger } from '@/lib/logger';
import { useSessionContext } from '@/context/SessionContext';

const logger = getLogger('SettingsPage');

interface ProviderCardProps {
  provider: AIServiceProvider;
  providerName: string;
  apiKey: string;
  baseUrl?: string;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
}

function ProviderCardBase({
  provider,
  providerName,
  apiKey,
  baseUrl,
  onPendingChange,
}: ProviderCardProps) {
  const [localApiKey, setLocalApiKey] = useState(apiKey || '');
  const [localBaseUrl, setLocalBaseUrl] = useState(baseUrl || '');

  // Debounce local edits into pending changes to avoid frequent context updates
  const debounceRef = useRef<number | null>(null);
  const schedulePending = useCallback(
    (patch: Partial<ServiceConfig>) => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
      // small debounce to group typing
      debounceRef.current = window.setTimeout(() => {
        onPendingChange(provider, patch);
      }, 350) as unknown as number;
    },
    [onPendingChange, provider],
  );

  return (
    <Card className="bg-background border shadow-sm min-w-0 w-full">
      <CardHeader className="pb-4">
        <CardTitle className="text-foreground text-base font-medium break-words">
          {providerName}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 min-w-0">
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            API Key
          </label>
          <Input
            type="password"
            placeholder={`Enter your ${providerName} API key`}
            value={localApiKey}
            onChange={(e) => {
              const v = e.target.value;
              setLocalApiKey(v);
              schedulePending({ apiKey: v });
            }}
            onBlur={() => onPendingChange(provider, { apiKey: localApiKey })}
            className="bg-background border text-foreground w-full"
          />
        </div>

        {provider === AIServiceProvider.Ollama && (
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 text-sm font-medium">
              Base URL
            </label>
            <Input
              type="url"
              placeholder="http://localhost:11434"
              value={localBaseUrl}
              onChange={(e) => {
                const v = e.target.value;
                setLocalBaseUrl(v);
                schedulePending({ baseUrl: v });
              }}
              onBlur={() =>
                onPendingChange(provider, { baseUrl: localBaseUrl })
              }
              className="bg-background border text-foreground w-full"
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}

const ProviderCard = React.memo(ProviderCardBase, (prev, next) => {
  return (
    prev.apiKey === next.apiKey && (prev.baseUrl || '') === (next.baseUrl || '')
  );
});

export default function SettingsPage() {
  const navigate = useNavigate();
  const {
    value: { serviceConfigs, windowSize, uiLanguage },
    update,
  } = useSettings();
  const { t } = useTranslation('common');

  // pending updates are collected here without causing re-renders
  const pendingRef = useRef<Partial<Record<AIServiceProvider, ServiceConfig>>>(
    {},
  );
  const [pendingCount, setPendingCount] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const sessionCtx = useSessionContext();

  // Local state for window size and language to prevent immediate context updates
  const [localWindowSize, setLocalWindowSize] = useState(windowSize);
  const [localLanguage, setLocalLanguage] = useState(uiLanguage);
  const otherPendingRef = useRef<{
    windowSize?: number;
    uiLanguage?: string;
  }>({});

  // Sync local state with context when context changes (e.g., after Apply or external updates)
  useEffect(() => {
    setLocalWindowSize(windowSize);
  }, [windowSize]);

  useEffect(() => {
    setLocalLanguage(uiLanguage);
  }, [uiLanguage]);

  const handlePendingChange = useCallback(
    (provider: AIServiceProvider, patch: Partial<ServiceConfig>) => {
      pendingRef.current = {
        ...(pendingRef.current || {}),
        [provider]: {
          ...(pendingRef.current[provider] || serviceConfigs[provider] || {}),
          ...patch,
        },
      } as Partial<Record<AIServiceProvider, ServiceConfig>>;
      setPendingCount(
        Object.keys(pendingRef.current).length +
          Object.keys(otherPendingRef.current).length,
      );
    },
    [serviceConfigs],
  );

  const handleWindowSizeChange = useCallback((value: number) => {
    setLocalWindowSize(value);
    otherPendingRef.current = {
      ...otherPendingRef.current,
      windowSize: value,
    };
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  }, []);

  const handleLanguageChange = useCallback((lng: string) => {
    setLocalLanguage(lng);
    otherPendingRef.current = {
      ...otherPendingRef.current,
      uiLanguage: lng,
    };
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  }, []);

  const flushPending = useCallback(async () => {
    const pending = pendingRef.current;
    const otherPending = otherPendingRef.current;
    if (
      (!pending || Object.keys(pending).length === 0) &&
      (!otherPending || Object.keys(otherPending).length === 0)
    ) {
      return;
    }
    try {
      // Prepare updates object
      const updates: Partial<{
        serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
        windowSize: number;
        uiLanguage: string;
      }> = {};

      // Merge pending service configs
      if (pending && Object.keys(pending).length > 0) {
        const merged: Record<AIServiceProvider, ServiceConfig> = {
          ...(serviceConfigs || {}),
        } as Record<AIServiceProvider, ServiceConfig>;

        for (const k of Object.keys(pending) as Array<AIServiceProvider>) {
          merged[k] = {
            ...(merged[k] || {}),
            ...(pending[k] as ServiceConfig),
          };
        }
        updates.serviceConfigs = merged;
      }

      // Add other pending changes
      if (otherPending.windowSize !== undefined) {
        updates.windowSize = otherPending.windowSize;
      }
      if (otherPending.uiLanguage !== undefined) {
        updates.uiLanguage = otherPending.uiLanguage;
        // Apply i18n language change when applying settings
        i18n.changeLanguage(otherPending.uiLanguage);
      }

      await update(updates);
      pendingRef.current = {};
      otherPendingRef.current = {};
      setPendingCount(0);
    } catch (e) {
      logger.error('Failed to apply pending settings', e);
      throw e;
    }
  }, [serviceConfigs, update]);

  const providerEntries = useMemo(() => {
    return Object.values(AIServiceProvider).filter(
      (p) => p !== AIServiceProvider.Empty,
    ) as AIServiceProvider[];
  }, []);

  return (
    <div className="p-6 text-gray-300 min-h-screen">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <BrainCircuit size={32} className="text-primary" />
          <div>
            <h1 className="text-2xl text-foreground font-semibold">
              {t('settings.title', 'Settings')}
            </h1>
            <p className="text-xs text-muted-foreground">
              LibrAgent v{__APP_VERSION__}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          {pendingCount > 0 && (
            <span className="text-sm text-yellow-400">
              {t('settings.unsaved', 'Unsaved')} ({pendingCount})
            </span>
          )}
          <Button onClick={() => navigate(-1)} variant="ghost">
            {t('common.close', 'Close')}
          </Button>
          <Button onClick={flushPending} disabled={pendingCount === 0}>
            {t('settings.applyChanges', 'Apply Changes')}
          </Button>
        </div>
      </div>

      <div className="max-w-5xl">
        <Tabs defaultValue="api-key" className="flex flex-col">
          <TabsList className="flex gap-2 overflow-x-auto mb-4">
            <TabsTrigger value="api-key">
              {t('settings.tabs.apiKey', 'API Key Settings')}
            </TabsTrigger>
            <TabsTrigger value="mcp-servers">
              {t('settings.tabs.mcpServers', 'MCP Servers')}
            </TabsTrigger>
            <TabsTrigger value="conversation-model">
              {t(
                'settings.tabs.conversationModel',
                'Conversation & Model Preferences',
              )}
            </TabsTrigger>
            <TabsTrigger value="data-reset">
              {t('settings.tabs.dataReset', 'Data & Reset')}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="api-key">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {providerEntries.map((provider) => {
                const cfg = serviceConfigs[provider] || {};
                const providerName =
                  provider.charAt(0).toUpperCase() + provider.slice(1);
                return (
                  <ProviderCard
                    key={provider}
                    provider={provider}
                    providerName={providerName}
                    apiKey={cfg.apiKey || ''}
                    baseUrl={cfg.baseUrl}
                    onPendingChange={handlePendingChange}
                  />
                );
              })}
            </div>
          </TabsContent>

          <TabsContent value="mcp-servers">
            <MCPServerManagement />
          </TabsContent>

          <TabsContent value="conversation-model">
            <div className="space-y-6">
              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.messageWindowSize', 'Message Window Size')}
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 50"
                  value={localWindowSize}
                  onChange={(e) =>
                    handleWindowSizeChange(parseInt(e.target.value, 10) || 0)
                  }
                  className="bg-background border text-foreground w-full max-w-xs"
                />
              </div>

              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.llmPreference', 'LLM Preference')}
                </label>
                <TerminalModelPicker />
              </div>

              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.language.label', 'Language')}
                </label>
                <select
                  className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
                  value={localLanguage}
                  onChange={(e) => handleLanguageChange(e.target.value)}
                >
                  <option value="en">
                    {t('settings.language.english', 'English')}
                  </option>
                  <option value="ko">
                    {t('settings.language.korean', 'Korean')}
                  </option>
                </select>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="data-reset">
            <div className="space-y-6">
              <Card className="bg-background border shadow-sm">
                <CardHeader className="pb-4">
                  <CardTitle className="text-foreground text-base font-medium">
                    {t('settings.dataReset.title', 'Data & Reset')}
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm text-muted-foreground">
                    {t(
                      'settings.dataReset.description',
                      'This will permanently delete all local sessions, their messages, and workspace file stores from the local database and native workspace directories. This action is destructive and cannot be undone.',
                    )}
                  </p>
                  <div className="flex items-center justify-start pt-4 gap-x-2">
                    <Button
                      type="button"
                      variant="destructive"
                      disabled={isDeleting}
                      onClick={async (e) => {
                        e.preventDefault();
                        e.stopPropagation();

                        // Open confirmation dialog
                        setConfirmOpen(true);
                      }}
                    >
                      {isDeleting && (
                        <LoadingSpinner size="sm" className="mr-2" />
                      )}
                      <span>
                        {isDeleting
                          ? t('settings.dataReset.deleting', 'Deleting...')
                          : t(
                              'settings.dataReset.clearAll',
                              'Clear All Sessions, Messages & Workspace',
                            )}
                      </span>
                    </Button>
                    <AlertDialog
                      open={confirmOpen}
                      onOpenChange={setConfirmOpen}
                    >
                      <AlertDialogContent>
                        <AlertDialogHeader>
                          <AlertDialogTitle>
                            {t(
                              'settings.dataReset.confirmTitle',
                              'Delete All Sessions, Messages & Workspace',
                            )}
                          </AlertDialogTitle>
                          <AlertDialogDescription>
                            {t(
                              'settings.dataReset.confirmDescription',
                              'This will permanently delete all local sessions, their messages, and workspace file stores from the local database and native workspace directories. This action cannot be undone. Are you sure you want to continue?',
                            )}
                          </AlertDialogDescription>
                        </AlertDialogHeader>
                        <AlertDialogFooter>
                          <AlertDialogCancel>
                            {t('common.cancel', 'Cancel')}
                          </AlertDialogCancel>
                          <AlertDialogAction
                            onClick={async () => {
                              setConfirmOpen(false);
                              setIsDeleting(true);
                              try {
                                await sessionCtx.clearAllSessions();
                                toast.success(
                                  t(
                                    'settings.dataReset.success',
                                    'All sessions, messages and workspace files have been successfully deleted.',
                                  ),
                                );
                              } catch (e) {
                                logger.error('Failed to clear sessions', e);
                                toast.error(
                                  t(
                                    'settings.dataReset.error',
                                    'Failed to clear sessions. See logs for details.',
                                  ),
                                );
                              } finally {
                                setIsDeleting(false);
                              }
                            }}
                          >
                            {t('common.delete', 'Delete')}
                          </AlertDialogAction>
                        </AlertDialogFooter>
                      </AlertDialogContent>
                    </AlertDialog>
                  </div>
                </CardContent>
              </Card>
            </div>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
