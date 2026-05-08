import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { mutate } from 'swr';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { AIServiceProvider } from '@/lib/ai-service';
import { useSettings } from '@/hooks/use-settings';
import i18n from '@/lib/i18n';
import type { ContextStrategy, ServiceConfig } from '@/context/SettingsContext';
import { getLogger } from '@/lib/logger';
import { dbUtils } from '@/lib/db/service';
import { restartApp } from '@/lib/backend';
import {
  factoryReset as backendFactoryReset,
  clearAllSessions as backendClearAllSessions,
} from '@/lib/backend/sessions';
import { useSettingsForm } from './useSettingsForm';

const logger = getLogger('useSettingsPageController');

const SETTINGS_TAB_VALUES = [
  'general',
  'ai-models',
  'chat-interface',
  'system',
  'advanced',
  'dev',
] as const;

export const PROVIDER_ENTRIES = Object.values(AIServiceProvider).filter(
  (provider) => provider !== AIServiceProvider.Empty,
) as AIServiceProvider[];

export type SettingsTabValue = (typeof SETTINGS_TAB_VALUES)[number];

function isSettingsTabValue(value: string): value is SettingsTabValue {
  return SETTINGS_TAB_VALUES.includes(value as SettingsTabValue);
}

async function invalidateModelCaches() {
  await Promise.all([
    mutate((key) => Array.isArray(key) && key[0] === 'local-models'),
    mutate((key) => Array.isArray(key) && key[0] === 'models'),
  ]);
}

export function useSettingsPageController() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { value: globalSettings } = useSettings();
  const { t } = useTranslation('common');
  const {
    formState,
    dirtyState,
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
  const isDirtyRef = useRef(isDirty);

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

  const networkSettingsChanged = useMemo(
    () =>
      formState.system.httpServerPort !==
        globalSettings.system.httpServerPort ||
      formState.system.httpServerExpose !==
        globalSettings.system.httpServerExpose,
    [
      formState.system.httpServerExpose,
      formState.system.httpServerPort,
      globalSettings.system.httpServerExpose,
      globalSettings.system.httpServerPort,
    ],
  );

  const changedSectionCount = useMemo(
    () =>
      Object.entries(dirtyState).filter(
        ([tab, isChanged]) =>
          isChanged && (import.meta.env.DEV || tab !== 'dev'),
      ).length,
    [dirtyState],
  );

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
    isDirtyRef.current = isDirty;
  }, [isDirty]);

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!isDirtyRef.current) {
        return;
      }

      event.preventDefault();
      event.returnValue = '';
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, []);

  const handleFactoryReset = useCallback(async () => {
    setIsResetting(true);
    try {
      try {
        await dbUtils.clearAllObjects();
        await dbUtils.clearAllSessions();
        await dbUtils.clearAllAssistants();
        await dbUtils.clearAllMCPServers();
        await dbUtils.clearAllPlaybooks();
      } catch (error) {
        logger.error('Failed to clear frontend DB during factory reset', error);
      }

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
    } catch (error) {
      logger.error('Failed to perform factory reset', error);
      toast.error(
        t(
          'settings.factoryReset.error',
          'Failed to perform factory reset. See logs for details.',
        ),
      );
    } finally {
      setIsResetting(false);
    }
  }, [t, triggerAppRestart]);

  const handleClearAllSessions = useCallback(async () => {
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
    } catch (error) {
      logger.error('Failed to clear sessions', error);
      toast.error(t('settings.dataReset.error', 'Failed to clear sessions.'));
    } finally {
      setIsDeleting(false);
    }
  }, [t, triggerAppRestart]);

  const handleSave = useCallback(async (): Promise<boolean> => {
    if (!isDirty) {
      return true;
    }

    setIsSaving(true);
    try {
      if (formState.uiLanguage !== globalSettings.uiLanguage) {
        await i18n.changeLanguage(formState.uiLanguage);
      }

      await save();
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
    } catch (error) {
      logger.error('Failed to save settings', error);
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
    [handleClearAllSessions, handleFactoryReset, isDeleting, isResetting],
  );

  const tabNavigationItems = useMemo(() => {
    const items: Array<{
      value: SettingsTabValue;
      label: string;
      isDirty: boolean;
      className?: string;
    }> = [
      {
        value: 'general',
        label: t('settings.tabs.general', 'General'),
        isDirty: dirtyState.general,
      },
      {
        value: 'ai-models',
        label: t('settings.tabs.aiModels', 'AI & Models'),
        isDirty: dirtyState['ai-models'],
      },
      {
        value: 'chat-interface',
        label: t('settings.tabs.chatInterface', 'Chat Interface'),
        isDirty: dirtyState['chat-interface'],
      },
      {
        value: 'system',
        label: t('settings.tabs.system', 'System'),
        isDirty: dirtyState.system,
      },
      {
        value: 'advanced',
        label: t('settings.tabs.advanced', 'Advanced'),
        isDirty: dirtyState.advanced,
      },
    ];

    if (import.meta.env.DEV) {
      items.push({
        value: 'dev',
        label: t('settings.tabs.dev', 'Dev'),
        isDirty: false,
        className: 'text-yellow-500',
      });
    }

    return items;
  }, [dirtyState, t]);

  return {
    activeTab,
    changedSectionCount,
    dangerZoneProps,
    formState,
    handleClose,
    handleContextStrategyChange,
    handleDefaultMaxOutputTokensChange,
    handleDiscard,
    handleDiscardAndLeave,
    handleFallbackModelChange,
    handleLanguageChange,
    handleMaxInputContextChange,
    handleMaxRetriesChange,
    handlePendingChange,
    handlePreferredModelChange,
    handleRetryDelayChange,
    handleSave,
    handleSaveAndLeave,
    handleTabChange,
    handleToolCallGroupVisibleCountChange,
    handleWindowSizeChange,
    isDeleting,
    isDirty,
    isDiscardDialogOpen,
    isLeaveDialogOpen,
    isSaving,
    networkSettingsChanged,
    setIsDiscardDialogOpen,
    setIsLeaveDialogOpen,
    systemSettingsProps,
    tabNavigationItems,
    updateAdvanced,
    updateDisplay,
  };
}
