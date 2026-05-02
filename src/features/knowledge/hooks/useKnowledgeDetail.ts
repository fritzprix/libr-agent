import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import useSWR from 'swr';
import {
  getGlobalKnowledgeDetail,
} from '@/lib/backend/knowledge';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useKnowledgeDetail');

export function useKnowledgeDetail(selectedId: number | null) {
  const { t } = useTranslation('common');

  const { data: detail = null, isValidating: isDetailLoading } = useSWR(
    selectedId !== null ? ['knowledge-detail', selectedId] : null,
    async ([, id]) => {
      try {
        return await getGlobalKnowledgeDetail(id as number);
      } catch (error) {
        logger.error('Failed to load knowledge detail', { error, selectedId: id });
        toast.error(
          t(
            'knowledge.loadDetailFailed',
            'Failed to load knowledge details.',
          ),
        );
        throw error;
      }
    },
    {
      revalidateOnFocus: false,
      shouldRetryOnError: false,
    }
  );

  return {
    detail,
    isDetailLoading,
  };
}
