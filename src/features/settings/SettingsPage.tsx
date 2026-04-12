import equal from 'fast-deep-equal';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { BrainCircuit, Loader2 } from 'lucide-react';
import { mutate } from 'swr';
import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '@/hooks/use-settings';
import { useTranslation } from 'react-i18next';
import i18n from '@/lib/i18n';
import type { ServiceConfig, ContextStrategy } from '@/context/SettingsContext';
import {
  Button,
  Badge,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
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
import SystemTab from './tabs/SystemTab';
import AdvancedTab from './tabs/AdvancedTab';
import DevTab from './tabs/DevTab';

const logger = getLogger('SettingsPage');

const invalidateModelCaches = async () => {
  await Promise.all([
    mutate((key) => Array.isArray(key) && key[0] === 'local-models'),
    mutate((key) => Array.isArray(key) && key[0] === 'models'),
  ]);
};

type SettingsTabValue =
  | 'general'
  | 'ai-models'
  | 'chat-interface'
  | 'system'
  | 'advanced'
  | 'dev';

function isSettingsTabValue(value: string): value is SettingsTabValue {
  return [
    'general',
    'ai-models',
    'chat-interface',
    'system',
    'advanced',
    'dev',
  ].includes(value);
}

export default function SettingsPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
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
    reset,
    save,
    isDirty,
  } = useSettingsForm();

  const [isDeleting, setIsDeleting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isDiscardDialogOpen, setIsDiscardDialogOpen] = useState(false);
  const [isLeaveDialogOpen, setIsLeaveDialogOpen] = useState(false);

  const activeTab = useMemo<SettingsTabValue>(() => {
    const tabParam = searchParams.get('tab');
    if (
      tabParam &&
      isSettingsTabValue(tabParam) &&
      (import.meta.env.DEV || tabParam !== 'dev')
    ) {
      return tabParam;
    }

    return 'general';
  }, [searchParams]);

  const networkSettingsChanged = useMemo(() => {
    return (
      formState.system.httpServerPort !==
        globalSettings.system.httpServerPort ||
      formState.system.httpServerExpose !==
        globalSettings.system.httpServerExpose
    );
  }, [
    formState.system.httpServerExpose,
    formState.system.httpServerPort,
    globalSettings.system.httpServerExpose,
    globalSettings.system.httpServerPort,
  ]);

  const tabDirtyState = useMemo(() => {
    const {
      diffContextLines: formDiffContextLines,
      ...formAdvancedWithoutDiff
    } = formState.advanced;
    const {
      diffContextLines: globalDiffContextLines,
      ...globalAdvancedWithoutDiff
    } = globalSettings.advanced;

    return {
      general:
        formState.uiLanguage !== globalSettings.uiLanguage ||
        !equal(formState.display, globalSettings.display),
      'ai-models': !equal(
        {
          serviceConfigs: formState.serviceConfigs,
          preferredModel: formState.preferredModel,
          fallbackModel: formState.fallbackModel,
          agentHubUrl: formState.agentHubUrl,
          maxRetries: formState.advanced.maxRetries,
          retryDelay: formState.advanced.retryDelay,
          defaultMaxOutputTokens: formState.advanced.defaultMaxOutputTokens,
        },
        {
          serviceConfigs: globalSettings.serviceConfigs,
          preferredModel: globalSettings.preferredModel,
          fallbackModel: globalSettings.fallbackModel,
          agentHubUrl: globalSettings.agentHubUrl,
          maxRetries: globalSettings.advanced.maxRetries,
          retryDelay: globalSettings.advanced.retryDelay,
          defaultMaxOutputTokens:
            globalSettings.advanced.defaultMaxOutputTokens,
        },
      ),
      'chat-interface': !equal(
        {
          contextStrategy: formState.contextStrategy,
          windowSize: formState.windowSize,
          maxInputContext: formState.maxInputContext,
          toolCallGroupVisibleCount: formState.toolCallGroupVisibleCount,
          diffContextLines: formDiffContextLines,
        },
        {
          contextStrategy: globalSettings.contextStrategy,
          windowSize: globalSettings.windowSize,
          maxInputContext: globalSettings.maxInputContext,
          toolCallGroupVisibleCount: globalSettings.toolCallGroupVisibleCount,
          diffContextLines: globalDiffContextLines,
        },
      ),
      system: !equal(
        {
          skillsDirectory: formState.system.skillsDirectory,
          maxFileUploadSizeMB: formState.system.maxFileUploadSizeMB,
          searchIndexFrequencyMinutes:
            formState.system.searchIndexFrequencyMinutes,
          webActionTimeoutSeconds: formState.system.webActionTimeoutSeconds,
          mcpServerStartupTimeoutSeconds:
            formState.system.mcpServerStartupTimeoutSeconds,
          mcpToolTimeoutSeconds: formState.system.mcpToolTimeoutSeconds,
          scheduledTaskMinimumIntervalMinutes:
            formState.system.scheduledTaskMinimumIntervalMinutes,
          maxScheduledTaskGroups: formState.system.maxScheduledTaskGroups,
          httpServerPort: formState.system.httpServerPort,
          httpServerExpose: formState.system.httpServerExpose,
        },
        {
          skillsDirectory: globalSettings.system.skillsDirectory,
          maxFileUploadSizeMB: globalSettings.system.maxFileUploadSizeMB,
          searchIndexFrequencyMinutes:
            globalSettings.system.searchIndexFrequencyMinutes,
          webActionTimeoutSeconds:
            globalSettings.system.webActionTimeoutSeconds,
          mcpServerStartupTimeoutSeconds:
            globalSettings.system.mcpServerStartupTimeoutSeconds,
          mcpToolTimeoutSeconds: globalSettings.system.mcpToolTimeoutSeconds,
          scheduledTaskMinimumIntervalMinutes:
            globalSettings.system.scheduledTaskMinimumIntervalMinutes,
          maxScheduledTaskGroups: globalSettings.system.maxScheduledTaskGroups,
          httpServerPort: globalSettings.system.httpServerPort,
          httpServerExpose: globalSettings.system.httpServerExpose,
        },
      ),
      advanced: !equal(
        {
          ...formAdvancedWithoutDiff,
          maxRetries: undefined,
          retryDelay: undefined,
          defaultMaxOutputTokens: undefined,
          shellIsolationLevel: formState.system.shellIsolationLevel,
        },
        {
          ...globalAdvancedWithoutDiff,
          maxRetries: undefined,
          retryDelay: undefined,
          defaultMaxOutputTokens: undefined,
          shellIsolationLevel: globalSettings.system.shellIsolationLevel,
        },
      ),
      dev: false,
    } satisfies Record<SettingsTabValue, boolean>;
  }, [formState, globalSettings]);

  const changedSectionCount = useMemo(() => {
    return Object.entries(tabDirtyState).filter(
      ([tab, isChanged]) => isChanged && (import.meta.env.DEV || tab !== 'dev'),
    ).length;
  }, [tabDirtyState]);

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

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!isDirty) {
        return;
      }

      event.preventDefault();
      event.returnValue = '';
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, [isDirty]);

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
    } finally {
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

  const handleSave = useCallback(async (): Promise<boolean> => {
    if (!isDirty) {
      return true;
    }

    setIsSaving(true);
    try {
      // Apply language change side effect
      if (formState.uiLanguage !== globalSettings.uiLanguage) {
        await i18n.changeLanguage(formState.uiLanguage);
      }

      await save();
      // Provider model lists depend on saved base URL / API key settings.
      // Revalidate those caches after save so pickers switch to the new endpoint
      // instead of waiting for a later manual refresh or SWR dedupe expiry.
      await invalidateModelCaches();

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
      return true;
    } catch (e) {
      logger.error('Failed to save settings', e);
      toast.error(t('settings.saveFailed', 'Failed to save settings'));
      return false;
    } finally {
      setIsSaving(false);
    }
  }, [
    formState.uiLanguage,
    globalSettings.uiLanguage,
    isDirty,
    networkSettingsChanged,
    save,
    t,
    triggerAppRestart,
  ]);

  const handleDiscard = useCallback(() => {
    reset();
    setIsDiscardDialogOpen(false);
  }, [reset]);

  const handleClose = useCallback(() => {
    if (isDirty) {
      setIsLeaveDialogOpen(true);
      return;
    }

    navigate(-1);
  }, [isDirty, navigate]);

  const handleSaveAndLeave = useCallback(async () => {
    const didSave = await handleSave();
    if (!didSave) {
      return;
    }

    setIsLeaveDialogOpen(false);
    navigate(-1);
  }, [handleSave, navigate]);

  const handleDiscardAndLeave = useCallback(() => {
    reset();
    setIsLeaveDialogOpen(false);
    navigate(-1);
  }, [navigate, reset]);

  const handleTabChange = useCallback(
    (value: string) => {
      if (
        !isSettingsTabValue(value) ||
        (!import.meta.env.DEV && value === 'dev')
      ) {
        return;
      }

      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.set('tab', value);
          return next;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

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

  const handleFallbackModelChange = useCallback(
    (model: string, provider: string) => {
      // Clear fallback if no model selected
      update(
        'fallbackModel',
        model ? { provider: provider as AIServiceProvider, model } : undefined,
      );
    },
    [update],
  );

  const handleWindowSizeChange = useCallback(
    (value: number) => update('windowSize', value),
    [update],
  );
  const handleContextStrategyChange = useCallback(
    (strategy: ContextStrategy) => update('contextStrategy', strategy),
    [update],
  );
  const handleMaxInputContextChange = useCallback(
    (value: number) => update('maxInputContext', value),
    [update],
  );
  const handleToolCallGroupVisibleCountChange = useCallback(
    (count: number) => update('toolCallGroupVisibleCount', count),
    [update],
  );
  const handleLanguageChange = useCallback(
    (lang: string) => update('uiLanguage', lang),
    [update],
  );
  const handleMaxRetriesChange = useCallback(
    (value: number) => updateAdvanced('maxRetries', value),
    [updateAdvanced],
  );
  const handleRetryDelayChange = useCallback(
    (value: number) => updateAdvanced('retryDelay', value),
    [updateAdvanced],
  );
  const handleDefaultMaxOutputTokensChange = useCallback(
    (value: number) => updateAdvanced('defaultMaxOutputTokens', value),
    [updateAdvanced],
  );

  // Memoize stable props objects
  const systemSettingsProps = useMemo(
    () => ({
      localSystemSettings: formState.system,
      onChange: updateSystem,
      networkSettingsChanged,
    }),
    [formState.system, networkSettingsChanged, updateSystem],
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
          <div className="flex items-center gap-2 flex-wrap justify-end">
            {isDirty && (
              <Badge
                variant="outline"
                className="border-warning/30 bg-warning/10 text-warning-foreground"
              >
                {t('settings.pendingChanges', {
                  count: changedSectionCount,
                  defaultValue: '{{count}} sections changed',
                })}
              </Badge>
            )}
            {networkSettingsChanged && (
              <Badge
                variant="outline"
                className="border-warning/30 bg-warning/10 text-warning-foreground"
              >
                {t(
                  'settings.system.restartRequired',
                  'Restart required after save',
                )}
              </Badge>
            )}
            <Button
              onClick={() => setIsDiscardDialogOpen(true)}
              variant="outline"
              className="h-9"
              disabled={!isDirty || isSaving}
            >
              {t('settings.discardChanges', 'Discard')}
            </Button>
            <Button
              onClick={handleClose}
              variant="ghost"
              className="h-9"
              disabled={isSaving}
            >
              {t('common.close', 'Close')}
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isDirty || isSaving}
              className="h-9 font-medium"
            >
              {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {isSaving
                ? t('settings.saving', 'Saving...')
                : t('settings.saveChanges', 'Save Changes')}
            </Button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          <Tabs
            value={activeTab}
            onValueChange={handleTabChange}
            className="flex flex-col min-h-full"
          >
            <TabsList className="sticky top-0 z-10 mb-4 flex gap-2 overflow-x-auto border border-border/60 bg-background/95 p-1 backdrop-blur supports-[backdrop-filter]:bg-background/80">
              <TabsTrigger value="general" className="gap-2">
                {t('settings.tabs.general', 'General')}
                {tabDirtyState.general && (
                  <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                )}
              </TabsTrigger>
              <TabsTrigger value="ai-models" className="gap-2">
                {t('settings.tabs.aiModels', 'AI & Models')}
                {tabDirtyState['ai-models'] && (
                  <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                )}
              </TabsTrigger>
              <TabsTrigger value="chat-interface" className="gap-2">
                {t('settings.tabs.chatInterface', 'Chat Interface')}
                {tabDirtyState['chat-interface'] && (
                  <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                )}
              </TabsTrigger>
              <TabsTrigger value="system" className="gap-2">
                {t('settings.tabs.system', 'System')}
                {tabDirtyState.system && (
                  <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                )}
              </TabsTrigger>
              <TabsTrigger value="advanced" className="gap-2">
                {t('settings.tabs.advanced', 'Advanced')}
                {tabDirtyState.advanced && (
                  <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                )}
              </TabsTrigger>
              {import.meta.env.DEV && (
                <TabsTrigger value="dev" className="gap-2 text-yellow-500">
                  {t('settings.tabs.dev', 'Dev')}
                </TabsTrigger>
              )}
            </TabsList>

            <TabsContent value="general">
              <GeneralTab
                localLanguage={formState.uiLanguage}
                onChange={handleLanguageChange}
                localDisplay={formState.display}
                onDisplaySettingsChange={updateDisplay}
              />
            </TabsContent>

            <TabsContent value="ai-models">
              <AIModelsTab
                serviceConfigs={formState.serviceConfigs}
                providerEntries={providerEntries}
                localPreferredModel={formState.preferredModel}
                localFallbackModel={formState.fallbackModel}
                localMaxRetries={formState.advanced.maxRetries}
                localRetryDelay={formState.advanced.retryDelay}
                localDefaultMaxOutputTokens={
                  formState.advanced.defaultMaxOutputTokens
                }
                onPendingChange={handlePendingChange}
                onPreferredModelChange={handlePreferredModelChange}
                onFallbackModelChange={handleFallbackModelChange}
                onMaxRetriesChange={handleMaxRetriesChange}
                onRetryDelayChange={handleRetryDelayChange}
                onDefaultMaxOutputTokensChange={
                  handleDefaultMaxOutputTokensChange
                }
              />
            </TabsContent>

            <TabsContent value="chat-interface">
              <ChatInterfaceTab
                localContextStrategy={formState.contextStrategy}
                localWindowSize={formState.windowSize}
                localMaxInputContext={formState.maxInputContext}
                localToolCallGroupVisibleCount={
                  formState.toolCallGroupVisibleCount
                }
                localAdvancedSettings={formState.advanced}
                onContextStrategyChange={handleContextStrategyChange}
                onWindowSizeChange={handleWindowSizeChange}
                onMaxInputContextChange={handleMaxInputContextChange}
                onToolCallGroupVisibleCountChange={
                  handleToolCallGroupVisibleCountChange
                }
                onAdvancedSettingsChange={updateAdvanced}
              />
            </TabsContent>

            <TabsContent value="system">
              <SystemTab systemSettingsProps={systemSettingsProps} />
            </TabsContent>

            <TabsContent value="advanced">
              <AdvancedTab
                localAdvancedSettings={formState.advanced}
                onChange={updateAdvanced}
                systemSettingsProps={systemSettingsProps}
                dangerZoneProps={dangerZoneProps}
              />
            </TabsContent>

            {import.meta.env.DEV && (
              <TabsContent value="dev">
                <DevTab serviceConfigs={formState.serviceConfigs} />
              </TabsContent>
            )}
          </Tabs>
        </div>
      </div>

      <Dialog open={isDiscardDialogOpen} onOpenChange={setIsDiscardDialogOpen}>
        <DialogContent showCloseButton={!isSaving}>
          <DialogHeader>
            <DialogTitle>
              {t('settings.discardTitle', 'Discard unsaved changes?')}
            </DialogTitle>
            <DialogDescription>
              {t(
                'settings.discardDescription',
                'This will revert every pending change on this page back to the last saved state.',
              )}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsDiscardDialogOpen(false)}
              disabled={isSaving}
            >
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDiscard}
              disabled={isSaving}
            >
              {t('settings.discardChanges', 'Discard')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={isLeaveDialogOpen} onOpenChange={setIsLeaveDialogOpen}>
        <DialogContent showCloseButton={!isSaving}>
          <DialogHeader>
            <DialogTitle>
              {t('settings.leaveTitle', 'Leave without saving?')}
            </DialogTitle>
            <DialogDescription>
              {t(
                'settings.leaveDescription',
                'You have unsaved changes. Save them before leaving, or discard them and leave this page.',
              )}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsLeaveDialogOpen(false)}
              disabled={isSaving}
            >
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDiscardAndLeave}
              disabled={isSaving}
            >
              {t('settings.discardAndLeave', 'Discard and Leave')}
            </Button>
            <Button onClick={handleSaveAndLeave} disabled={isSaving}>
              {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('settings.saveAndLeave', 'Save and Leave')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
