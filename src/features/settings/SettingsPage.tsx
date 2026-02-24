import { useCallback, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { BrainCircuit } from 'lucide-react';
import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '@/hooks/use-settings';
import { useTranslation } from 'react-i18next';
import i18n from '@/lib/i18n';
import type {
  ServiceConfig,
} from '@/context/SettingsContext';
import {
  Button,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { dbUtils } from '@/lib/db/service';
import { restartApp } from '@/lib/backend';
import {
  factoryReset as backendFactoryReset,
  clearAllSessions as backendClearAllSessions,
} from '@/lib/backend/sessions';
import { useSettingsForm } from './hooks/useSettingsForm';
import GeneralTab from './tabs/GeneralTab';
import AIModelsTab from './tabs/AIModelsTab';
import ChatInterfaceTab from './tabs/ChatInterfaceTab';
import AdvancedTab from './tabs/AdvancedTab';

const logger = getLogger('SettingsPage');

export default function SettingsPage() {
  const navigate = useNavigate();
  // We still need global settings to detect network changes
  const { value: globalSettings } = useSettings();
  const { t } = useTranslation('common');

  const {
    formState,
    update,
    updateServiceConfig,
    updateAdvanced,
    updateDisplay,
    updateSystem,
    save,
    isDirty,
  } = useSettingsForm();

  const [isDeleting, setIsDeleting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

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

  const handleSave = async () => {
    if (!isDirty) return;

    setIsSaving(true);
    try {
      // Detect changes that require restart
      const networkSettingsChanged =
        formState.system.httpServerPort !==
          globalSettings.system.httpServerPort ||
        formState.system.httpServerExpose !==
          globalSettings.system.httpServerExpose;

      // Apply language change side effect
      if (formState.uiLanguage !== globalSettings.uiLanguage) {
        i18n.changeLanguage(formState.uiLanguage);
      }

      await save();

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
      toast.success(t('settings.saved', 'Settings saved successfully'));
    } catch (e) {
      logger.error('Failed to save settings', e);
      toast.error(t('common.error', 'Failed to save settings'));
    } finally {
      setIsSaving(false);
    }
  };

  // Adapters for Tab callbacks
  const handlePendingChange = useCallback(
    (provider: AIServiceProvider, patch: Partial<ServiceConfig>) => {
      updateServiceConfig(provider, patch);
    },
    [updateServiceConfig],
  );

  const handlePreferredModelChange = useCallback(
    (model: string, provider: string) => {
      update('preferredModel', {
        provider: provider as AIServiceProvider,
        model,
      });
    },
    [update],
  );

  const handleWindowSizeChange = useCallback(
    (value: number) => update('windowSize', value),
    [update],
  );
  const handleToolCallGroupVisibleCountChange = useCallback(
    (count: number) => update('toolCallGroupVisibleCount', count),
    [update],
  );
  const handleAgentHubUrlChange = useCallback(
    (url: string) => update('agentHubUrl', url),
    [update],
  );
  const handleLanguageChange = useCallback(
    (lang: string) => update('uiLanguage', lang),
    [update],
  );
  const handleSkillsDirectoryChange = useCallback(
    (path: string) => updateSystem('skillsDirectory', path),
    [updateSystem],
  );

  // Memoize stable props objects
  const systemSettingsProps = useMemo(
    () => ({
      localSystemSettings: formState.system,
      onChange: updateSystem,
    }),
    [formState.system, updateSystem],
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

  const providerEntries = useMemo(() => {
    return Object.values(AIServiceProvider).filter(
      (p) => p !== AIServiceProvider.Empty,
    ) as AIServiceProvider[];
  }, []);

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
              <BrainCircuit size={28} />
            </div>
            <div>
              <h1 className="text-2xl text-foreground font-semibold tracking-tight">
                {t('settings.title', 'Settings')}
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                LibrAgent v{__APP_VERSION__}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {isDirty && (
              <span className="text-sm text-warning font-medium">
                {t('settings.unsaved', 'Unsaved')}
              </span>
            )}
            <Button
              onClick={() => navigate(-1)}
              variant="ghost"
              className="h-9"
            >
              {t('common.close', 'Close')}
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isDirty || isSaving}
              className="h-9 font-medium"
            >
              {isSaving ? 'Saving...' : t('settings.applyChanges', 'Apply Changes')}
            </Button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
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
              <TabsTrigger value="advanced">
                {t('settings.tabs.advanced', 'Advanced')}
              </TabsTrigger>
            </TabsList>

            <TabsContent value="general">
              <GeneralTab
                localLanguage={formState.uiLanguage}
                onChange={handleLanguageChange}
                skillsDirectory={formState.system.skillsDirectory}
                onSkillsDirectoryChange={handleSkillsDirectoryChange}
              />
            </TabsContent>

            <TabsContent value="ai-models">
              <AIModelsTab
                serviceConfigs={formState.serviceConfigs}
                providerEntries={providerEntries}
                localPreferredModel={formState.preferredModel}
                localAgentHubUrl={formState.agentHubUrl || ''}
                onPendingChange={handlePendingChange}
                onPreferredModelChange={handlePreferredModelChange}
                onAgentHubUrlChange={handleAgentHubUrlChange}
              />
            </TabsContent>

            <TabsContent value="chat-interface">
              <ChatInterfaceTab
                localWindowSize={formState.windowSize}
                localToolCallGroupVisibleCount={
                  formState.toolCallGroupVisibleCount
                }
                localAdvancedSettings={formState.advanced}
                localDisplay={formState.display}
                onWindowSizeChange={handleWindowSizeChange}
                onToolCallGroupVisibleCountChange={
                  handleToolCallGroupVisibleCountChange
                }
                onAdvancedSettingsChange={updateAdvanced}
                onDisplaySettingsChange={updateDisplay}
              />
            </TabsContent>

            <TabsContent value="advanced">
              <AdvancedTab
                localAdvancedSettings={formState.advanced}
                onChange={updateAdvanced}
                systemSettingsProps={systemSettingsProps}
                dangerZoneProps={dangerZoneProps}
              />
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </div>
  );
}
