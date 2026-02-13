import { useState, useEffect } from 'react';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  getWorkspaceOverride,
  setWorkspaceOverride,
  cancelWorkspaceOverride,
} from '@/lib/backend';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentWorkspacePanel');

export function useWorkspaceOverride(onWorkspaceChanged: () => void) {
  const { session } = useAgentSessionState();
  const [workspaceOverride, setWorkspaceOverridePath] = useState<string>('');
  const [isOverrideActive, setIsOverrideActive] = useState(false);

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

  const handleSetOverride = async () => {
    if (!workspaceOverride.trim() || !session?.id) return;

    try {
      await setWorkspaceOverride(session.id, workspaceOverride);
      setIsOverrideActive(true);
      toast.success('Workspace override set successfully');
      onWorkspaceChanged();
    } catch (error) {
      logger.error('Failed to set workspace override', error);
      toast.error(`Failed to set override: ${error}`);
    }
  };

  const handleCancelOverride = async () => {
    if (!session?.id) return;

    try {
      await cancelWorkspaceOverride(session.id);
      setWorkspaceOverridePath('');
      setIsOverrideActive(false);
      toast.success('Workspace override cancelled');
      onWorkspaceChanged();
    } catch (error) {
      logger.error('Failed to cancel workspace override', error);
      toast.error(`Failed to cancel override: ${error}`);
    }
  };

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Workspace Directory',
      });

      if (selected && typeof selected === 'string') {
        setWorkspaceOverridePath(selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
      toast.error(`Failed to open folder dialog: ${error}`);
    }
  };

  return {
    workspaceOverride,
    isOverrideActive,
    handleSetOverride,
    handleCancelOverride,
    handleBrowseFolder,
  };
}
