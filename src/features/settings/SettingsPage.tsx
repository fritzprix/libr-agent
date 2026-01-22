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
import type {
  ServiceConfig,
  AdvancedSettings,
  DisplaySettings,
  SystemSettings,
} from '@/context/SettingsContext';
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
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
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
import { dbUtils } from '@/lib/db/service';
import {
  factoryReset as backendFactoryReset,
  clearAllSessions as backendClearAllSessions,
} from '@/lib/backend/sessions';
import { useDebounce } from '@/hooks/useDebounce';

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

  // Use debounce hook for pending changes
  const { debounced: schedulePending } = useDebounce(
    (patch: Partial<ServiceConfig>) => {
      onPendingChange(provider, patch);
    },
    350,
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

        {(provider === AIServiceProvider.Ollama ||
          provider === AIServiceProvider.OpenAI) && (
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
    value: {
      serviceConfigs,
      windowSize,
      uiLanguage,
      toolCallGroupVisibleCount,
      agentHubUrl,
      advanced,
      display,
      system,
      preferredModel,
    },
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

  const [isResetting, setIsResetting] = useState(false);
  const [resetConfirmOpen, setResetConfirmOpen] = useState(false);

  const handleFactoryReset = async () => {
    setResetConfirmOpen(false);
    setIsResetting(true);
    try {
      // 1. Clear ALL frontend data
      try {
        await dbUtils.clearAllObjects();
        await dbUtils.clearAllSessions();
        await dbUtils.clearAllAssistants();
        await dbUtils.clearAllMCPServers();
        await dbUtils.clearAllPlaybooks();
      } catch (e) {
        logger.error('Failed to clear frontend DB during factory reset', e);
        // Continue to backend reset
      }

      // 2. Perform factory reset on backend
      await backendFactoryReset();

      // 3. Restore defaults
      // await LocalDatabase.getInstance().ensureDefaultAssistants();

      toast.success(
        t(
          'settings.factoryReset.success',
          'Factory reset complete. The application will reload.',
        ),
      );

      // Reload to ensure fresh state
      setTimeout(() => {
        window.location.reload();
      }, 1500);
    } catch (e) {
      logger.error('Failed to perform factory reset', e);
      toast.error(
        t(
          'settings.factoryReset.error',
          'Failed to perform factory reset. See logs for details.',
        ),
      );
      setIsResetting(false);
    }
  };

  // Local state for window size and language to prevent immediate context updates
  const [localWindowSize, setLocalWindowSize] = useState(windowSize);
  const [localLanguage, setLocalLanguage] = useState(uiLanguage);
  const [localToolCallGroupVisibleCount, setLocalToolCallGroupVisibleCount] =
    useState(toolCallGroupVisibleCount);
  const [localAgentHubUrl, setLocalAgentHubUrl] = useState(agentHubUrl || '');
  const [localAdvancedSettings, setLocalAdvancedSettings] =
    useState<AdvancedSettings>(
      advanced || {
        maxRetries: 1,
        retryDelay: 5000,
        circuitBreakerThreshold: 3,
        diffContextLines: 3,
      },
    );
  const [localDisplay, setLocalDisplay] = useState<DisplaySettings>(
    display || {
      metricDisplayMode: 'inline',
      prefillDisplayFormat: 'time',
      showTokenSpeed: true,
      compactMetrics: false,
    },
  );
  const [localSystemSettings, setLocalSystemSettings] =
    useState<SystemSettings>(
      system || {
        maxFileUploadSizeMB: 50,
        workspaceCapacityMB: 10,
        webActionTimeoutSeconds: 30,
        searchIndexFrequencyMinutes: 5,
        activeSessionRetentionHours: 24,
      },
    );

  const [localPreferredModel, setLocalPreferredModel] = useState<{
    provider: AIServiceProvider;
    model: string;
  }>(
    preferredModel || {
      provider: AIServiceProvider.OpenAI,
      model: 'gpt-4o',
    },
  );

  const otherPendingRef = useRef<{
    windowSize?: number;
    uiLanguage?: string;
    toolCallGroupVisibleCount?: number;
    agentHubUrl?: string;
    advanced?: AdvancedSettings;
    display?: DisplaySettings;
    system?: SystemSettings;
    preferredModel?: { provider: AIServiceProvider; model: string };
  }>({});

  // Sync local state with context when context changes (e.g., after Apply or external updates)
  useEffect(() => {
    setLocalWindowSize(windowSize);
  }, [windowSize]);

  useEffect(() => {
    setLocalLanguage(uiLanguage);
  }, [uiLanguage]);

  useEffect(() => {
    setLocalToolCallGroupVisibleCount(toolCallGroupVisibleCount);
  }, [toolCallGroupVisibleCount]);

  useEffect(() => {
    setLocalDisplay(display);
  }, [display]);

  useEffect(() => {
    if (system) {
      setLocalSystemSettings(system);
    }
  }, [system]);

  useEffect(() => {
    setLocalAgentHubUrl(agentHubUrl || '');
  }, [agentHubUrl]);

  useEffect(() => {
    if (advanced) {
      setLocalAdvancedSettings(advanced);
    }
  }, [advanced]);

  useEffect(() => {
    if (preferredModel) {
      setLocalPreferredModel(preferredModel);
    }
  }, [preferredModel]);

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

  const handleLanguageChange = (lang: string) => {
    setLocalLanguage(lang);
    otherPendingRef.current.uiLanguage = lang;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handleToolCallGroupVisibleCountChange = (count: number) => {
    setLocalToolCallGroupVisibleCount(count);
    otherPendingRef.current.toolCallGroupVisibleCount = count;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handleAgentHubUrlChange = (url: string) => {
    setLocalAgentHubUrl(url);
    otherPendingRef.current.agentHubUrl = url;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handleAdvancedSettingsChange = (
    key: keyof AdvancedSettings,
    value: number,
  ) => {
    const newSettings = { ...localAdvancedSettings, [key]: value };
    setLocalAdvancedSettings(newSettings);
    otherPendingRef.current.advanced = newSettings;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handleDisplaySettingsChange = (
    key: keyof DisplaySettings,
    value: string | boolean,
  ) => {
    const newSettings = { ...localDisplay, [key]: value };
    setLocalDisplay(newSettings);
    otherPendingRef.current.display = newSettings;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handleSystemSettingsChange = (
    key: keyof SystemSettings,
    value: number,
  ) => {
    const newSettings = { ...localSystemSettings, [key]: value };
    setLocalSystemSettings(newSettings);
    otherPendingRef.current.system = newSettings;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  };

  const handlePreferredModelChange = useCallback(
    (model: string, provider: string) => {
      const newVal = { provider: provider as AIServiceProvider, model };
      setLocalPreferredModel(newVal);
      otherPendingRef.current.preferredModel = newVal;
      setPendingCount(
        Object.keys(pendingRef.current).length +
          Object.keys(otherPendingRef.current).length,
      );
    },
    [],
  );

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
      const updates: Partial<{
        serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
        windowSize: number;
        uiLanguage: string;
        toolCallGroupVisibleCount: number;
        advanced: AdvancedSettings;
        display: DisplaySettings;
        system: SystemSettings;
        preferredModel: { provider: AIServiceProvider; model: string };
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
      if (otherPending.toolCallGroupVisibleCount !== undefined) {
        updates.toolCallGroupVisibleCount =
          otherPending.toolCallGroupVisibleCount;
      }
      if (otherPending.advanced) {
        updates.advanced = otherPending.advanced;
      }
      if (otherPending.display) {
        updates.display = otherPending.display;
      }
      if (otherPending.system) {
        updates.system = otherPending.system;
      }
      if (otherPending.preferredModel) {
        updates.preferredModel = otherPending.preferredModel;
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
    <div className="p-6 text-muted-foreground min-h-screen">
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
        <Tabs defaultValue="general" className="flex flex-col">
          <TabsList className="flex gap-2 overflow-x-auto mb-4">
            <TabsTrigger value="general">
              {t('settings.tabs.general', 'General')}
            </TabsTrigger>
            <TabsTrigger value="ai-models">
              {t('settings.tabs.aiModels', 'AI & Models')}
            </TabsTrigger>
            <TabsTrigger value="chat-interface">
              {t('settings.tabs.chatInterface', 'Chat Interface')}
            </TabsTrigger>
            <TabsTrigger value="mcp-servers">
              {t('settings.tabs.mcpServers', 'MCP Servers')}
            </TabsTrigger>
            <TabsTrigger value="advanced">
              {t('settings.tabs.advanced', 'Advanced')}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="general">
            <div className="space-y-6">
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

          <TabsContent value="ai-models">
            <div className="space-y-8">
              {/* API Keys Section */}
              <div className="space-y-4">
                <h3 className="text-lg font-medium text-foreground">
                  {t('settings.aiModels.apiKeys', 'Provider API Keys')}
                </h3>
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
              </div>

              {/* Model Preference Section */}
              <div className="space-y-4">
                <h3 className="text-lg font-medium text-foreground">
                  {t('settings.aiModels.preferences', 'Model Preferences')}
                </h3>
                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    {t('settings.llmPreference', 'Default LLM')}
                  </label>
                  <AgentModelPicker
                    currentModel={localPreferredModel.model}
                    currentProvider={localPreferredModel.provider}
                    onConfigUpdate={handlePreferredModelChange}
                    className="w-full max-w-sm"
                  />
                  {/* Note: TerminalModelPicker had different UX. AgentModelPicker is more compact. */}
                </div>
              </div>

              {/* Agent Hub Section */}
              <div className="space-y-4">
                <h3 className="text-lg font-medium text-foreground">
                  {t('settings.aiModels.agentHub', 'Agent Hub')}
                </h3>
                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    Agent Hub URL
                  </label>
                  <Input
                    type="url"
                    placeholder="https://api.agenthub.com"
                    value={localAgentHubUrl}
                    onChange={(e) => handleAgentHubUrlChange(e.target.value)}
                    className="bg-background border text-foreground w-full"
                  />
                  <p className="text-xs text-muted-foreground mt-1">
                    URL of the remote Agent Hub server. If set, assistants will
                    be synced with this server.
                  </p>
                </div>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="chat-interface">
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
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
                  <p className="text-xs text-muted-foreground mt-1">
                    {t(
                      'settings.messageWindowSizeDescription',
                      'Number of messages to keep in conversation history',
                    )}
                  </p>
                </div>

                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    {t(
                      'settings.toolCallGroupVisibleCount',
                      'Tool Calls Visible Count',
                    )}
                  </label>
                  <Input
                    type="number"
                    placeholder="e.g., 4"
                    min={1}
                    max={20}
                    value={localToolCallGroupVisibleCount}
                    onChange={(e) =>
                      handleToolCallGroupVisibleCountChange(
                        parseInt(e.target.value, 10) || 4,
                      )
                    }
                    className="bg-background border text-foreground w-full max-w-xs"
                  />
                  <p className="text-xs text-muted-foreground mt-1">
                    {t(
                      'settings.toolCallGroupVisibleCountDescription',
                      'Number of tool calls to show in collapsed group view',
                    )}
                  </p>
                </div>

                <div className="min-w-0">
                  <label className="block text-muted-foreground mb-2 font-medium">
                    Diff Context Lines
                  </label>
                  <Input
                    type="number"
                    placeholder="e.g., 3"
                    min={1}
                    max={10}
                    value={localAdvancedSettings.diffContextLines ?? 3}
                    onChange={(e) =>
                      handleAdvancedSettingsChange(
                        'diffContextLines',
                        parseInt(e.target.value, 10) || 3,
                      )
                    }
                    className="bg-background border text-foreground w-full max-w-xs"
                  />
                  <p className="text-xs text-muted-foreground mt-1">
                    Number of context lines to show in file edit diffs (1-10).
                  </p>
                </div>
              </div>

              <div className="border-t pt-6 mt-6">
                <h3 className="text-lg font-medium text-foreground mb-4">
                  {t('settings.display.metricsTitle', 'Performance Metrics')}
                </h3>
                <div className="space-y-6">
                  <div className="min-w-0">
                    <label className="block text-muted-foreground mb-2 font-medium">
                      {t(
                        'settings.display.metricDisplayMode',
                        'Metric Display Mode',
                      )}
                    </label>
                    <select
                      className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
                      value={localDisplay.metricDisplayMode}
                      onChange={(e) =>
                        handleDisplaySettingsChange(
                          'metricDisplayMode',
                          e.target.value as 'tooltip' | 'inline',
                        )
                      }
                    >
                      <option value="inline">
                        {t(
                          'settings.display.inline',
                          'Inline (show in message)',
                        )}
                      </option>
                      <option value="tooltip">
                        {t(
                          'settings.display.tooltip',
                          'Tooltip (hover to see)',
                        )}
                      </option>
                    </select>
                    <p className="text-xs text-muted-foreground mt-1">
                      {t(
                        'settings.display.metricDisplayModeDescription',
                        'Choose how token metrics are displayed in chat messages',
                      )}
                    </p>
                  </div>

                  <div className="min-w-0">
                    <label className="block text-muted-foreground mb-2 font-medium">
                      {t(
                        'settings.display.prefillDisplayFormat',
                        'Prefill Performance Format',
                      )}
                    </label>
                    <select
                      className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
                      value={localDisplay.prefillDisplayFormat}
                      onChange={(e) =>
                        handleDisplaySettingsChange(
                          'prefillDisplayFormat',
                          e.target.value as 'time' | 'tokensPerSecond',
                        )
                      }
                    >
                      <option value="time">
                        {t(
                          'settings.display.time',
                          'Time to First Token (e.g., 245ms)',
                        )}
                      </option>
                      <option value="tokensPerSecond">
                        {t(
                          'settings.display.tokensPerSecond',
                          'Tokens Per Second (e.g., 520 tok/s)',
                        )}
                      </option>
                    </select>
                    <p className="text-xs text-muted-foreground mt-1">
                      {t(
                        'settings.display.prefillDisplayFormatDescription',
                        'Choose how prefill performance is displayed',
                      )}
                    </p>
                  </div>

                  <div className="flex flex-col gap-4">
                    <div className="min-w-0">
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={localDisplay.showTokenSpeed}
                          onChange={(e) =>
                            handleDisplaySettingsChange(
                              'showTokenSpeed',
                              e.target.checked,
                            )
                          }
                          className="w-4 h-4"
                        />
                        <span className="text-muted-foreground font-medium">
                          {t(
                            'settings.display.showTokenSpeed',
                            'Show Token Speed',
                          )}
                        </span>
                      </label>
                      <p className="text-xs text-muted-foreground mt-1 ml-6">
                        {t(
                          'settings.display.showTokenSpeedDescription',
                          'Display generation speed (tokens per second) in metrics',
                        )}
                      </p>
                    </div>

                    <div className="min-w-0">
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={localDisplay.compactMetrics}
                          onChange={(e) =>
                            handleDisplaySettingsChange(
                              'compactMetrics',
                              e.target.checked,
                            )
                          }
                          className="w-4 h-4"
                        />
                        <span className="text-muted-foreground font-medium">
                          {t(
                            'settings.display.compactMetrics',
                            'Compact Metrics',
                          )}
                        </span>
                      </label>
                      <p className="text-xs text-muted-foreground mt-1 ml-6">
                        {t(
                          'settings.display.compactMetricsDescription',
                          'Use compact display format for token metrics',
                        )}
                      </p>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="mcp-servers">
            <MCPServerManagement />
          </TabsContent>

          <TabsContent value="advanced">
            <div className="space-y-6">
              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.advanced.maxRetries', 'Max Retry Attempts')}
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 1"
                  min={0}
                  max={5}
                  value={localAdvancedSettings.maxRetries}
                  onChange={(e) =>
                    handleAdvancedSettingsChange(
                      'maxRetries',
                      parseInt(e.target.value, 10) || 0,
                    )
                  }
                  className="bg-background border text-foreground w-full max-w-xs"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.advanced.maxRetriesDescription',
                    'Maximum number of retries for failed AI requests.',
                  )}
                </p>
              </div>

              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.advanced.retryDelay', 'Retry Delay (ms)')}
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 5000"
                  min={1000}
                  step={1000}
                  value={localAdvancedSettings.retryDelay}
                  onChange={(e) =>
                    handleAdvancedSettingsChange(
                      'retryDelay',
                      parseInt(e.target.value, 10) || 5000,
                    )
                  }
                  className="bg-background border text-foreground w-full max-w-xs"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.advanced.retryDelayDescription',
                    'Delay in milliseconds between retry attempts.',
                  )}
                </p>
              </div>

              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 font-medium">
                  {t('settings.advanced.circuitBreaker', 'Tool Loop Threshold')}
                </label>
                <Input
                  type="number"
                  placeholder="e.g., 3"
                  min={1}
                  max={10}
                  value={localAdvancedSettings.circuitBreakerThreshold}
                  onChange={(e) =>
                    handleAdvancedSettingsChange(
                      'circuitBreakerThreshold',
                      parseInt(e.target.value, 10) || 3,
                    )
                  }
                  className="bg-background border text-foreground w-full max-w-xs"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.advanced.circuitBreakerDescription',
                    'Number of repeated errors or tool calls before triggering the circuit breaker.',
                  )}
                </p>
              </div>

              {/* System & Performance */}
              <div className="border-t pt-8 mt-4">
                <h3 className="text-lg font-medium text-foreground mb-4">
                  {t('settings.system.title', 'System & Performance')}
                </h3>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
                  {/* File & Workspace */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-medium text-foreground">
                      {t('settings.system.fileWorkspace', 'File & Workspace')}
                    </h4>
                    {/* Max File Upload Size */}
                    <div className="min-w-0">
                      <label className="block text-muted-foreground mb-2 font-medium">
                        {t(
                          'settings.system.maxFileUploadSize',
                          'Max File Upload Size (MB)',
                        )}
                      </label>
                      <Input
                        type="number"
                        placeholder="e.g., 50"
                        min={1}
                        value={localSystemSettings.maxFileUploadSizeMB}
                        onChange={(e) =>
                          handleSystemSettingsChange(
                            'maxFileUploadSizeMB',
                            parseInt(e.target.value, 10) || 50,
                          )
                        }
                        className="bg-background border text-foreground w-full max-w-xs"
                      />
                      <p className="text-xs text-muted-foreground mt-1">
                        {t(
                          'settings.system.maxFileUploadSizeDescription',
                          'Maximum size for a single file upload. Increase if you often work with large documents.',
                        )}
                      </p>
                    </div>

                    {/* Workspace Capacity */}
                    <div className="min-w-0">
                      <label className="block text-muted-foreground mb-2 font-medium">
                        {t(
                          'settings.system.workspaceCapacity',
                          'Workspace Capacity (MB)',
                        )}
                      </label>
                      <Input
                        type="number"
                        placeholder="e.g., 10"
                        min={1}
                        value={localSystemSettings.workspaceCapacityMB}
                        onChange={(e) =>
                          handleSystemSettingsChange(
                            'workspaceCapacityMB',
                            parseInt(e.target.value, 10) || 10,
                          )
                        }
                        className="bg-background border text-foreground w-full max-w-xs"
                      />
                      <p className="text-xs text-muted-foreground mt-1">
                        {t(
                          'settings.system.workspaceCapacityDescription',
                          "Total limit for your current workspace's text content.",
                        )}
                      </p>
                    </div>
                  </div>

                  {/* Background Tasks */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-medium text-foreground">
                      {t('settings.system.backgroundTasks', 'Background Tasks')}
                    </h4>
                    {/* Search Index Frequency */}
                    <div className="min-w-0">
                      <label className="block text-muted-foreground mb-2 font-medium">
                        {t(
                          'settings.system.searchIndexFrequency',
                          'Search Index Frequency (Min)',
                        )}
                      </label>
                      <Input
                        type="number"
                        placeholder="e.g., 5"
                        min={1}
                        value={localSystemSettings.searchIndexFrequencyMinutes}
                        onChange={(e) =>
                          handleSystemSettingsChange(
                            'searchIndexFrequencyMinutes',
                            parseInt(e.target.value, 10) || 5,
                          )
                        }
                        className="bg-background border text-foreground w-full max-w-xs"
                      />
                      <p className="text-xs text-muted-foreground mt-1">
                        {t(
                          'settings.system.searchIndexFrequencyDescription',
                          'How often the AI updates its memory search. Faster updates keep search fresh but use more battery/CPU.',
                        )}
                      </p>
                    </div>

                    {/* Web Action Timeout */}
                    <div className="min-w-0">
                      <label className="block text-muted-foreground mb-2 font-medium">
                        {t(
                          'settings.system.webActionTimeout',
                          'Web Action Timeout (Sec)',
                        )}
                      </label>
                      <Input
                        type="number"
                        placeholder="e.g., 30"
                        min={5}
                        value={localSystemSettings.webActionTimeoutSeconds}
                        onChange={(e) =>
                          handleSystemSettingsChange(
                            'webActionTimeoutSeconds',
                            parseInt(e.target.value, 10) || 30,
                          )
                        }
                        className="bg-background border text-foreground w-full max-w-xs"
                      />
                      <p className="text-xs text-muted-foreground mt-1">
                        {t(
                          'settings.system.webActionTimeoutDescription',
                          'How long the AI waits for a webpage to load or a click to finish.',
                        )}
                      </p>
                    </div>

                    {/* Session Retention */}
                    <div className="min-w-0">
                      <label className="block text-muted-foreground mb-2 font-medium">
                        {t(
                          'settings.system.activeSessionRetention',
                          'Keep Active Sessions For (Hours)',
                        )}
                      </label>
                      <Input
                        type="number"
                        placeholder="e.g., 24"
                        min={1}
                        value={localSystemSettings.activeSessionRetentionHours}
                        onChange={(e) =>
                          handleSystemSettingsChange(
                            'activeSessionRetentionHours',
                            parseInt(e.target.value, 10) || 24,
                          )
                        }
                        className="bg-background border text-foreground w-full max-w-xs"
                      />
                      <p className="text-xs text-muted-foreground mt-1">
                        {t(
                          'settings.system.activeSessionRetentionDescription',
                          'How long to keep session data in fast memory.',
                        )}
                      </p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Danger Zone */}
              <div className="border-t pt-8 mt-4">
                <h3 className="text-lg font-medium text-destructive mb-4 flex items-center gap-2">
                  ⚠️ Danger Zone
                </h3>
                <div className="space-y-6">
                  <Card className="bg-background border border-destructive/20 shadow-sm">
                    <CardHeader className="pb-4">
                      <CardTitle className="text-foreground text-base font-medium">
                        {t('settings.dataReset.title', 'Data & Reset')}
                      </CardTitle>
                    </CardHeader>
                    <CardContent>
                      <p className="text-sm text-muted-foreground">
                        {t(
                          'settings.dataReset.description',
                          'This will permanently delete all local sessions, their messages, and workspace file stores.',
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
                                  'Clear All Sessions & Workspace',
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
                                  'This will permanently delete all local sessions, their messages, and workspace file stores. This action cannot be undone. Are you sure you want to continue?',
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
                                    await dbUtils.clearAllSessions();
                                    await backendClearAllSessions();
                                    toast.success(
                                      t(
                                        'settings.dataReset.success',
                                        'All sessions have been deleted.',
                                      ),
                                    );
                                    setTimeout(() => {
                                      window.location.reload();
                                    }, 1000);
                                  } catch (e) {
                                    logger.error('Failed to clear sessions', e);
                                    toast.error(
                                      t(
                                        'settings.dataReset.error',
                                        'Failed to clear sessions.',
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

                  <Card className="bg-background border border-destructive/20 shadow-sm">
                    <CardHeader className="pb-4">
                      <CardTitle className="text-foreground text-base font-medium">
                        {t('settings.factoryReset.title', 'Factory Reset')}
                      </CardTitle>
                    </CardHeader>
                    <CardContent>
                      <p className="text-sm text-muted-foreground">
                        {t(
                          'settings.factoryReset.description',
                          'This will perform a complete factory reset. It deletes ALL data.',
                        )}
                      </p>
                      <div className="flex items-center justify-start pt-4 gap-x-2">
                        <Button
                          type="button"
                          variant="destructive"
                          disabled={isResetting || isDeleting}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            setResetConfirmOpen(true);
                          }}
                        >
                          {isResetting && (
                            <LoadingSpinner size="sm" className="mr-2" />
                          )}
                          <span>
                            {isResetting
                              ? t(
                                  'settings.factoryReset.resetting',
                                  'Resetting...',
                                )
                              : t(
                                  'settings.factoryReset.button',
                                  'Reset All Data & Settings',
                                )}
                          </span>
                        </Button>
                        <AlertDialog
                          open={resetConfirmOpen}
                          onOpenChange={setResetConfirmOpen}
                        >
                          <AlertDialogContent>
                            <AlertDialogHeader>
                              <AlertDialogTitle>
                                {t(
                                  'settings.factoryReset.confirmTitle',
                                  'Factory Reset Confirmation',
                                )}
                              </AlertDialogTitle>
                              <AlertDialogDescription>
                                {t(
                                  'settings.factoryReset.confirmDescription',
                                  'This will permanently delete ALL data including sessions, assistants, MCP servers, and playbooks. Are you sure?',
                                )}
                              </AlertDialogDescription>
                            </AlertDialogHeader>
                            <AlertDialogFooter>
                              <AlertDialogCancel>
                                {t('common.cancel', 'Cancel')}
                              </AlertDialogCancel>
                              <AlertDialogAction onClick={handleFactoryReset}>
                                {t(
                                  'settings.factoryReset.confirmButton',
                                  'Reset Everything',
                                )}
                              </AlertDialogAction>
                            </AlertDialogFooter>
                          </AlertDialogContent>
                        </AlertDialog>
                      </div>
                    </CardContent>
                  </Card>
                </div>
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
