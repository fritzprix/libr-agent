import { useState, useCallback, useEffect, useRef } from 'react';
import { getLogger } from '@/lib/logger';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { SkillMetadata } from '@/types/skills';
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

  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [loadingSkills, setLoadingSkills] = useState<Record<string, boolean>>({});

  const lastDraftIdRef = useRef<string | undefined>(draft?.id);

  const fetchSkills = useCallback(
    async (id: string) => {
      if (lastDraftIdRef.current !== id) return;
      setIsLoading(true);
      try {
        const result = await getAggregatedSkills(id);
        if (lastDraftIdRef.current === id) {
          setSkills(result);
        }
      } catch (error) {
        logger.error('Failed to fetch skills:', error);
        toast.error(t('skills.fetchFailed'));
      } finally {
        if (lastDraftIdRef.current === id) {
          setIsLoading(false);
        }
      }
    },
    [t]
  );

  useEffect(() => {
    lastDraftIdRef.current = draft?.id;
    if (draft?.id) {
      fetchSkills(draft.id);
    } else {
      setSkills([]);
    }
    return () => {
      lastDraftIdRef.current = undefined;
    };
  }, [draft?.id, fetchSkills]);

  const handleOverride = async (skillName: string) => {
    const id = draft?.id;
    if (!id || loadingSkills[skillName]) return;
    setLoadingSkills((prev) => ({ ...prev, [skillName]: true }));
    try {
      await copyGlobalToAssistant(id, skillName);
      toast.success(t('skills.overrideSuccess'));
      if (lastDraftIdRef.current === id) {
        await fetchSkills(id);
      }
    } catch (error) {
      logger.error('Failed to override skill:', error);
      toast.error(t('skills.overrideFailed'));
    } finally {
      if (lastDraftIdRef.current === id) {
        setLoadingSkills((prev) => ({ ...prev, [skillName]: false }));
      }
    }
  };

  const handleRevert = async (skillName: string) => {
    const id = draft?.id;
    if (!id || loadingSkills[skillName]) return;
    setLoadingSkills((prev) => ({ ...prev, [skillName]: true }));
    try {
      await deleteAssistantSkill(id, skillName);
      toast.success(t('skills.revertSuccess'));
      if (lastDraftIdRef.current === id) {
        await fetchSkills(id);
      }
    } catch (error) {
      logger.error('Failed to revert skill:', error);
      toast.error(t('skills.revertFailed'));
    } finally {
      if (lastDraftIdRef.current === id) {
        setLoadingSkills((prev) => ({ ...prev, [skillName]: false }));
      }
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
    const id = draft?.id;
    if (!id || isResetting) return;

    setIsResetting(true);
    try {
      await resetAssistantSkills(id);
      toast.success(t('skills.resetSuccess'));
      if (lastDraftIdRef.current === id) {
        await fetchSkills(id);
        onSuccess?.();
      }
    } catch (error) {
      logger.error('Failed to reset skills:', error);
      toast.error(t('skills.resetFailed'));
    } finally {
      if (lastDraftIdRef.current === id) {
        setIsResetting(false);
      }
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
