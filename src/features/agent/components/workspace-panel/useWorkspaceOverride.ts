import { useState, useEffect, useCallback } from 'react';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  getWorkspaceOverride,
  setWorkspaceOverride,
  cancelWorkspaceOverride,
} from '@/lib/backend';
import { open } from '@tauri-apps/plugin-dialog';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';

const logger = getLogger('useWorkspaceOverride');

export function useWorkspaceOverride(onOverrideChanged: () => void) {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();

  const [workspaceOverride, setWorkspaceOverridePath] = useState<string>('');
  const [isOverrideActive, setIsOverrideActive] = useState(false);
  const [isSettingOverride, setIsSettingOverride] = useState(false);
  const [isCancelingOverride, setIsCancelingOverride] = useState(false);
  const [isBrowsing, setIsBrowsing] = useState(false);

  // Load current workspace override
  useEffect(() => {
    if (session?.id) {
      getWorkspaceOverride(session.id)
        .then((path) => {
          if (path) {
            setWorkspaceOverridePath(path);
            setIsOverrideActive(true);
          } else {
            setWorkspaceOverridePath('');
            setIsOverrideActive(false);
          }
        })
        .catch((err) => logger.error('Failed to load workspace override', err));
    }
  }, [session?.id]);

  const applyWorkspaceOverride = useCallback(
    async (overridePath: string) => {
      if (!overridePath.trim() || !session?.id || isSettingOverride) return;

      setIsSettingOverride(true);
      try {
        await setWorkspaceOverride(session.id, overridePath);
        setWorkspaceOverridePath(overridePath);
        setIsOverrideActive(true);
        toast.success(t('agent.workspace.setOverrideSuccess'));
        onOverrideChanged();
      } catch (error) {
        logger.error('Failed to set workspace override', error);
        toast.error(t('agent.workspace.setOverrideError', { error }));
      } finally {
        setIsSettingOverride(false);
      }
    },
    [isSettingOverride, onOverrideChanged, session?.id, t],
  );

  const handleSetOverride = useCallback(async () => {
    await applyWorkspaceOverride(workspaceOverride);
  }, [applyWorkspaceOverride, workspaceOverride]);

  const handleCancelOverride = async () => {
    if (!session?.id || isCancelingOverride) return;

    setIsCancelingOverride(true);
    try {
      await cancelWorkspaceOverride(session.id);
      setWorkspaceOverridePath('');
      setIsOverrideActive(false);
      toast.success(t('agent.workspace.cancelOverrideSuccess'));
      onOverrideChanged();
    } catch (error) {
      logger.error('Failed to cancel workspace override', error);
      toast.error(t('agent.workspace.cancelOverrideError', { error }));
    } finally {
      setIsCancelingOverride(false);
    }
  };

  const handleBrowseFolder = async () => {
    if (isBrowsing) return;
    setIsBrowsing(true);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('agent.workspace.selectDirectoryTitle'),
      });

      if (selected && typeof selected === 'string') {
        setWorkspaceOverridePath(selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
      toast.error(t('agent.workspace.openFolderDialogError', { error }));
    } finally {
      setIsBrowsing(false);
    }
  };

  return {
    workspaceOverride,
    setWorkspaceOverridePath,
    isOverrideActive,
    isSettingOverride,
    isCancelingOverride,
    isBrowsing,
    applyWorkspaceOverride,
    handleSetOverride,
    handleCancelOverride,
    handleBrowseFolder,
  };
}
