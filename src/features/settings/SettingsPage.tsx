import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { toast } from 'sonner';
import { MCPServerManagement } from './MCPServerManagement';
import { getLogger } from '@/lib/logger';
import { dbUtils } from '@/lib/db/service';
import { restartApp } from '@/lib/backend';
import {
  factoryReset as backendFactoryReset,
  clearAllSessions as backendClearAllSessions,
} from '@/lib/backend/sessions';
import GeneralTab from './tabs/GeneralTab';
import AIModelsTab from './tabs/AIModelsTab';
import ChatInterfaceTab from './tabs/ChatInterfaceTab';
import AdvancedTab from './tabs/AdvancedTab';

const logger = getLogger('SettingsPage');

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

  // Store serviceConfigs in ref to avoid callback recreation
  const serviceConfigsRef = useRef(serviceConfigs);
  useEffect(() => {
    serviceConfigsRef.current = serviceConfigs;
  }, [serviceConfigs]);

  // pending updates are collected here without causing re-renders
  const pendingRef = useRef<Partial<Record<AIServiceProvider, ServiceConfig>>>(
    {},
  );
  const [pendingCount, setPendingCount] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);

  const triggerAppRestart = useCallback(() => {
    if (import.meta.env.DEV) {
      window.location.reload();
      return;
    }

    restartApp().catch((error: unknown) => {
      logger.error('Failed to restart app', error);
      toast.error(
        t(
          'settings.system.networkRestartFailed',
          'Failed to restart the app. Please restart manually.',
        ),
      );
    });
  }, [t]);

  const handleFactoryReset = async () => {
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

      toast.success(
        t(
          'settings.factoryReset.success',
          'Factory reset complete. Restart required to fully apply changes.',
        ),
        {
          action: {
            label: t('common.restartNow', 'Restart now'),
            onClick: triggerAppRestart,
          },
        },
      );
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

  const handleClearAllSessions = async () => {
    setIsDeleting(true);
    try {
      await dbUtils.clearAllSessions();
      await backendClearAllSessions();
      toast.success(
        t(
          'settings.dataReset.success',
          'All sessions have been deleted. Restart recommended.',
        ),
        {
          action: {
            label: t('common.restartNow', 'Restart now'),
            onClick: triggerAppRestart,
          },
        },
      );
    } catch (e) {
      logger.error('Failed to clear sessions', e);
      toast.error(t('settings.dataReset.error', 'Failed to clear sessions.'));
    } finally {
      setIsDeleting(false);
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
        defaultMaxOutputTokens: 8192,
        defaultSessionMaxDepth: 0,
        defaultSessionMaxFanout: 0,
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
        httpServerPort: 3030,
        httpServerExpose: false,
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
      const currentConfig = serviceConfigsRef.current[provider] || {};
      pendingRef.current = {
        ...(pendingRef.current || {}),
        [provider]: {
          ...(pendingRef.current[provider] || currentConfig),
          ...patch,
        },
      } as Partial<Record<AIServiceProvider, ServiceConfig>>;
      setPendingCount(
        Object.keys(pendingRef.current).length +
          Object.keys(otherPendingRef.current).length,
      );
    },
    [], // No dependencies - using refs
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

  const handleLanguageChange = useCallback((lang: string) => {
    setLocalLanguage(lang);
    otherPendingRef.current.uiLanguage = lang;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  }, []);

  const handleToolCallGroupVisibleCountChange = useCallback((count: number) => {
    setLocalToolCallGroupVisibleCount(count);
    otherPendingRef.current.toolCallGroupVisibleCount = count;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  }, []);

  const handleAgentHubUrlChange = useCallback((url: string) => {
    setLocalAgentHubUrl(url);
    otherPendingRef.current.agentHubUrl = url;
    setPendingCount(
      Object.keys(pendingRef.current).length +
        Object.keys(otherPendingRef.current).length,
    );
  }, []);

  const handleAdvancedSettingsChange = useCallback(
    (key: keyof AdvancedSettings, value: number) => {
      setLocalAdvancedSettings((prev) => {
        const newSettings = { ...prev, [key]: value };
        otherPendingRef.current.advanced = newSettings;
        setPendingCount(
          Object.keys(pendingRef.current).length +
            Object.keys(otherPendingRef.current).length,
        );
        return newSettings;
      });
    },
    [],
  );

  const handleDisplaySettingsChange = useCallback(
    (key: keyof DisplaySettings, value: string | boolean) => {
      setLocalDisplay((prev) => {
        const newSettings = { ...prev, [key]: value };
        otherPendingRef.current.display = newSettings;
        setPendingCount(
          Object.keys(pendingRef.current).length +
            Object.keys(otherPendingRef.current).length,
        );
        return newSettings;
      });
    },
    [],
  );

  const handleSystemSettingsChange = useCallback(
    (key: keyof SystemSettings, value: number | string | boolean) => {
      setLocalSystemSettings((prev) => {
        const newSettings = { ...prev, [key]: value };
        otherPendingRef.current.system = newSettings;
        setPendingCount(
          Object.keys(pendingRef.current).length +
            Object.keys(otherPendingRef.current).length,
        );
        return newSettings;
      });
    },
    [],
  );

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
      let networkSettingsChanged = false;

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
          ...(serviceConfigsRef.current || {}),
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
        networkSettingsChanged =
          otherPending.system.httpServerPort !== system.httpServerPort ||
          otherPending.system.httpServerExpose !== system.httpServerExpose;
      }
      if (otherPending.preferredModel) {
        updates.preferredModel = otherPending.preferredModel;
      }

      await update(updates);

      if (networkSettingsChanged) {
        toast.info(
          t(
            'settings.system.networkRestartNotice',
            'Changes to HTTP server network settings are applied after restarting the app.',
          ),
          {
            action: {
              label: t('common.restartNow', 'Restart now'),
              onClick: triggerAppRestart,
            },
          },
        );
      }

      pendingRef.current = {};
      otherPendingRef.current = {};
      setPendingCount(0);
    } catch (e) {
      logger.error('Failed to apply pending settings', e);
      throw e;
    }
  }, [
    system.httpServerExpose,
    system.httpServerPort,
    t,
    triggerAppRestart,
    update,
  ]);

  const providerEntries = useMemo(() => {
    return Object.values(AIServiceProvider).filter(
      (p) => p !== AIServiceProvider.Empty,
    ) as AIServiceProvider[];
  }, []);

  // Memoize stable props objects to prevent child re-renders
  const systemSettingsProps = useMemo(
    () => ({
      localSystemSettings,
      onChange: handleSystemSettingsChange,
    }),
    [localSystemSettings, handleSystemSettingsChange],
  );

  const dangerZoneProps = useMemo(
    () => ({
      isDeleting,
      isResetting,
      onDelete: handleClearAllSessions,
      onReset: handleFactoryReset,
    }),
    [isDeleting, isResetting],
  );

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
            <span className="text-sm text-warning">
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
            <GeneralTab
              localLanguage={localLanguage}
              onChange={handleLanguageChange}
              skillsDirectory={localSystemSettings.skillsDirectory}
              onSkillsDirectoryChange={(path) =>
                handleSystemSettingsChange('skillsDirectory', path)
              }
            />
          </TabsContent>

          <TabsContent value="ai-models">
            <AIModelsTab
              serviceConfigs={serviceConfigs}
              providerEntries={providerEntries}
              localPreferredModel={localPreferredModel}
              localAgentHubUrl={localAgentHubUrl}
              onPendingChange={handlePendingChange}
              onPreferredModelChange={handlePreferredModelChange}
              onAgentHubUrlChange={handleAgentHubUrlChange}
            />
          </TabsContent>

          <TabsContent value="chat-interface">
            <ChatInterfaceTab
              localWindowSize={localWindowSize}
              localToolCallGroupVisibleCount={localToolCallGroupVisibleCount}
              localAdvancedSettings={localAdvancedSettings}
              localDisplay={localDisplay}
              onWindowSizeChange={handleWindowSizeChange}
              onToolCallGroupVisibleCountChange={
                handleToolCallGroupVisibleCountChange
              }
              onAdvancedSettingsChange={handleAdvancedSettingsChange}
              onDisplaySettingsChange={handleDisplaySettingsChange}
            />
          </TabsContent>

          <TabsContent value="mcp-servers">
            <MCPServerManagement />
          </TabsContent>

          <TabsContent value="advanced">
            <AdvancedTab
              localAdvancedSettings={localAdvancedSettings}
              onChange={handleAdvancedSettingsChange}
              systemSettingsProps={systemSettingsProps}
              dangerZoneProps={dangerZoneProps}
            />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
