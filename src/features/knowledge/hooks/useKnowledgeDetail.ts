import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  getGlobalKnowledgeDetail,
  type KnowledgeChunkDetail,
} from '@/lib/backend/knowledge';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useKnowledgeDetail');

export function useKnowledgeDetail(selectedId: number | null) {
  const { t } = useTranslation('common');
  const [detail, setDetail] = useState<KnowledgeChunkDetail | null>(null);
  const [isDetailLoading, setIsDetailLoading] = useState(false);

  useEffect(() => {
    if (selectedId === null) {
      setDetail(null);
      setIsDetailLoading(false);
      return;
    }

    let cancelled = false;

    const loadDetail = async () => {
      setIsDetailLoading(true);
      try {
        const response = await getGlobalKnowledgeDetail(selectedId);
        if (!cancelled) {
          setDetail(response);
        }
      } catch (error) {
        logger.error('Failed to load knowledge detail', { error, selectedId });
        if (!cancelled) {
          toast.error(
            t(
              'knowledge.loadDetailFailed',
              'Failed to load knowledge details.',
            ),
          );
          setDetail(null);
        }
      } finally {
        if (!cancelled) {
          setIsDetailLoading(false);
        }
      }
    };

    void loadDetail();

    return () => {
      cancelled = true;
    };
  }, [selectedId, t]);

  return {
    detail,
    isDetailLoading,
  };
}
