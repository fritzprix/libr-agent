import useSWR from 'swr';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { getGlobalKnowledgeDetail } from '@/lib/backend/knowledge';

const logger = getLogger('useKnowledgeDetail');

export function useKnowledgeDetail(selectedId: number | null) {
  const { t } = useTranslation('common');

  const { data, error, isLoading, mutate } = useSWR(
    selectedId ? ['knowledgeDetail', selectedId] : null,
    async ([, id]) => {
      return getGlobalKnowledgeDetail(id as number);
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to load knowledge detail', { selectedId, err });
        toast.error(
          t('knowledge.loadDetailFailed', 'Failed to load knowledge details.'),
        );
      },
    }
  );

  return {
    detail: data || null,
    isDetailLoading: isLoading,
    error,
    mutateDetail: mutate,
  };
}
