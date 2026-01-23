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
import {
  factoryReset as backendFactoryReset,
  clearAllSessions as backendClearAllSessions,
} from '@/lib/backend/sessions';
import { GeneralTab } from './tabs/GeneralTab';
import { AIModelsTab } from './tabs/AIModelsTab';
import { ChatInterfaceTab } from './tabs/ChatInterfaceTab';
import { AdvancedTab } from './tabs/AdvancedTab';

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

  // pending updates are collected here without causing re-renders
  const pendingRef = useRef<Partial<Record<AIServiceProvider, ServiceConfig>>>(
    {},
  );
  const [pendingCount, setPendingCount] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);

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

  const handleClearAllSessions = async () => {
    setIsDeleting(true);
    try {
      await dbUtils.clearAllSessions();
      await backendClearAllSessions();
      toast.success(
        t('settings.dataReset.success', 'All sessions have been deleted.'),
      );
      setTimeout(() => {
        window.location.reload();
      }, 1000);
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
            <GeneralTab
              localLanguage={localLanguage}
              onChange={handleLanguageChange}
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
              systemSettingsProps={{
                localSystemSettings: localSystemSettings,
                onChange: handleSystemSettingsChange,
              }}
              dangerZoneProps={{
                isDeleting,
                isResetting,
                onDelete: handleClearAllSessions,
                onReset: handleFactoryReset,
              }}
            />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
