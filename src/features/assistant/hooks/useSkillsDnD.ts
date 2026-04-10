import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getLogger } from '@/lib/logger';
import { useDnDContext } from '@/context/DnDContext';
import { importAssistantSkills } from '@/lib/backend/skills';
import { toast } from 'sonner';

const logger = getLogger('useSkillsDnD');

export function useSkillsDnD(
  draftId: string | undefined,
  cardRef: React.RefObject<HTMLDivElement>,
  fetchSkills: () => void,
) {
  const { t } = useTranslation('common');
  const { subscribe } = useDnDContext();
  const [isDragging, setIsDragging] = useState(false);

  useEffect(() => {
    if (!draftId || !cardRef.current) return;

    const unlisten = subscribe(
      cardRef,
      async (event, payload) => {
        if (event === 'drag-over') {
          setIsDragging(true);
        } else if (event === 'leave') {
          setIsDragging(false);
        } else if (
          event === 'drop' &&
          payload.paths &&
          payload.paths.length > 0
        ) {
          setIsDragging(false);
          const filePath = payload.paths[0];
          const toastId = toast.loading(t('skills.importing'));

          try {
            await importAssistantSkills(draftId, filePath);
            toast.success(t('skills.importSuccess'), { id: toastId });
            fetchSkills();
          } catch (error) {
            logger.error('Failed to import skills:', error);
            toast.error(`${t('skills.importFailed')}: ${error}`, {
              id: toastId,
            });
          }
        }
      },
      { priority: 1 }, // Higher priority to capture events
    );

    return () => {
      unlisten();
    };
  }, [draftId, fetchSkills, subscribe, t, cardRef]);

  return { isDragging };
}
