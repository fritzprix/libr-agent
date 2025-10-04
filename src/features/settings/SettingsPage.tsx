import React, { useCallback, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '@/hooks/use-settings';
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
  TerminalModelPicker,
} from '@/components/ui';
import { getLogger } from '@/lib/logger';
import { deleteContentStore } from '@/lib/rust-backend-client';
import { dbUtils } from '@/lib/db/service';

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
      <CardHeader className="pb-2">
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
    value: { serviceConfigs, windowSize },
    update,
  } = useSettings();

  // pending updates are collected here without causing re-renders
  const pendingRef = useRef<Partial<Record<AIServiceProvider, ServiceConfig>>>(
    {},
  );
  const [pendingCount, setPendingCount] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);

  const handlePendingChange = useCallback(
    (provider: AIServiceProvider, patch: Partial<ServiceConfig>) => {
      pendingRef.current = {
        ...(pendingRef.current || {}),
        [provider]: {
          ...(pendingRef.current[provider] || serviceConfigs[provider] || {}),
          ...patch,
        },
      } as Partial<Record<AIServiceProvider, ServiceConfig>>;
      setPendingCount(Object.keys(pendingRef.current).length);
    },
    [serviceConfigs],
  );

  const flushPending = useCallback(async () => {
    const pending = pendingRef.current;
    if (!pending || Object.keys(pending).length === 0) return;
    try {
      // Merge pending into the current serviceConfigs and write once
      const merged: Record<AIServiceProvider, ServiceConfig> = {
        ...(serviceConfigs || {}),
      } as Record<AIServiceProvider, ServiceConfig>;

      for (const k of Object.keys(pending) as Array<AIServiceProvider>) {
        merged[k] = {
          ...(merged[k] || {}),
          ...(pending[k] as ServiceConfig),
        };
      }

      await update({ serviceConfigs: merged });
      pendingRef.current = {};
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
        <h1 className="text-2xl text-foreground font-semibold">Settings</h1>
        <div className="flex items-center gap-3">
          {pendingCount > 0 && (
            <span className="text-sm text-yellow-400">
              Unsaved ({pendingCount})
            </span>
          )}
          <Button onClick={() => navigate(-1)} variant="ghost">
            Close
          </Button>
          <Button onClick={flushPending} disabled={pendingCount === 0}>
            Apply Changes
          </Button>
        </div>
      </div>

      <div className="max-w-5xl">
        <Tabs defaultValue="api-key" className="flex flex-col">
          <TabsList className="flex gap-2 overflow-x-auto mb-4">
            <TabsTrigger value="api-key">API Key Settings</TabsTrigger>
            <TabsTrigger value="conversation-model">
              Conversation & Model Preferences
            </TabsTrigger>
            <TabsTrigger value="data-reset">Data & Reset</TabsTrigger>
          </TabsList>

          <TabsContent value="api-key">
            <div className="flex flex-col gap-4">
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

          <TabsContent value="conversation-model">
            <div className="space-y-6">
              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  Message Window Size
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 50"
                  value={windowSize}
                  onChange={(e) =>
                    update({ windowSize: parseInt(e.target.value, 10) || 0 })
                  }
                  className="bg-background border text-foreground w-full max-w-xs"
                />
              </div>

              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  LLM Preference
                </label>
                <TerminalModelPicker />
              </div>
            </div>
          </TabsContent>

          <TabsContent value="data-reset">
            <div className="space-y-6">
              <Card className="bg-background border shadow-sm">
                <CardHeader className="pb-2">
                  <CardTitle className="text-foreground text-base font-medium">
                    Data & Reset
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm text-muted-foreground">
                    This will permanently delete all local sessions, their
                    messages, and workspace file stores from the local database
                    and native workspace directories. This action is destructive
                    and cannot be undone.
                  </p>
                  <div className="pt-4">
                    <Button
                      variant="destructive"
                      disabled={isDeleting}
                      onClick={async (e) => {
                        e.preventDefault(); // Prevent any default behavior

                        const ok = window.confirm(
                          'Delete ALL local sessions, messages, content stores and workspace files? This cannot be undone.',
                        );
                        if (!ok) return;

                        setIsDeleting(true);
                        try {
                          // Get all sessions from frontend DB
                          const sessions = await dbUtils.getAllSessions();

                          for (const s of sessions) {
                            const sid = s.id;
                            try {
                              // Remove content-store artifacts (SQLite + search index)
                              await deleteContentStore(sid);
                            } catch (e) {
                              logger.warn('deleteContentStore failed', e);
                            }

                            try {
                              // Remove DB rows and native workspace directory
                              await dbUtils.clearSessionAndWorkspace(sid);
                            } catch (e) {
                              logger.warn('clearSessionAndWorkspace failed', e);
                            }
                          }

                          // Show success message before navigation
                          window.alert(
                            'All sessions, messages and workspace files have been successfully deleted.',
                          );

                          // Navigate back after showing success message
                          navigate(-1);
                        } catch (e) {
                          logger.error('Failed to clear sessions', e);
                          // Let user know
                          window.alert(
                            'Failed to clear sessions. See logs for details.',
                          );
                        } finally {
                          setIsDeleting(false);
                        }
                      }}
                    >
                      {isDeleting
                        ? 'Deleting...'
                        : 'Clear All Sessions, Messages & Workspace'}
                    </Button>
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
