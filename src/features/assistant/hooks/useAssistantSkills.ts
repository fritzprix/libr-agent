import { useState } from 'react';
import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import {
  getAggregatedSkills,
  copyGlobalToAssistant,
  deleteAssistantSkill,
  resetAssistantSkills,
} from '@/lib/backend/skills';

const logger = getLogger('useAssistantSkills');

export function useAssistantSkills() {
  const { t } = useTranslation('common');
  const { draft, update } = useEditor<Assistant>();

  const [isResetting, setIsResetting] = useState(false);
  const [loadingSkills, setLoadingSkills] = useState<Record<string, boolean>>(
    {},
  );

  // Fetch skills using SWR
  const {
    data: skills = [],
    isValidating: isLoading,
    mutate: fetchSkills,
  } = useSWR(
    draft?.id ? ['assistant-skills', draft.id] : null,
    async ([, id]) => {
      return await getAggregatedSkills(id as string);
    },
    { revalidateOnFocus: false, shouldRetryOnError: false },
  );

  const handleOverride = async (skillName: string) => {
    if (!draft?.id || loadingSkills[skillName]) return;
    setLoadingSkills((prev) => ({ ...prev, [skillName]: true }));
    try {
      await copyGlobalToAssistant(draft.id, skillName);
      toast.success(t('skills.overrideSuccess'));
      await fetchSkills();
    } catch (error) {
      logger.error('Failed to override skill:', error);
      toast.error(t('skills.overrideFailed'));
    } finally {
      setLoadingSkills((prev) => ({ ...prev, [skillName]: false }));
    }
  };

  const handleRevert = async (skillName: string) => {
    if (!draft?.id || loadingSkills[skillName]) return;
    setLoadingSkills((prev) => ({ ...prev, [skillName]: true }));
    try {
      await deleteAssistantSkill(draft.id, skillName);
      toast.success(t('skills.revertSuccess'));
      await fetchSkills();
    } catch (error) {
      logger.error('Failed to revert skill:', error);
      toast.error(t('skills.revertFailed'));
    } finally {
      setLoadingSkills((prev) => ({ ...prev, [skillName]: false }));
    }
  };

  const handleToggle = (skillName: string, checked: boolean) => {
    update((draft) => {
      if (!draft.disabledSkills) draft.disabledSkills = [];

      if (checked) {
        // Enable: remove from disabledSkills
        draft.disabledSkills = draft.disabledSkills.filter(
          (name) => name !== skillName,
        );
      } else {
        // Disable: add to disabledSkills
        if (!draft.disabledSkills.includes(skillName)) {
          draft.disabledSkills.push(skillName);
        }
      }
    });
  };

  const confirmReset = async (onSuccess?: () => void) => {
    if (!draft?.id || isResetting) return;

    setIsResetting(true);
    try {
      await resetAssistantSkills(draft.id);
      toast.success(t('skills.resetSuccess'));
      await fetchSkills();
      onSuccess?.();
    } catch (error) {
      logger.error('Failed to reset skills:', error);
      toast.error(t('skills.resetFailed'));
    } finally {
      setIsResetting(false);
    }
  };

  return {
    skills,
    isLoading,
    isResetting,
    loadingSkills,
    fetchSkills,
    handleOverride,
    handleRevert,
    handleToggle,
    confirmReset,
  };
}
