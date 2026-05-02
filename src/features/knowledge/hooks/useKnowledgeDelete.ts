import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  deleteGlobalKnowledge,
  type KnowledgeChunkListItem,
} from '@/lib/backend/knowledge';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useKnowledgeDelete');

interface UseKnowledgeDeleteOptions {
  onDeleted: () => void;
  selectedItem: KnowledgeChunkListItem | null;
}

export function useKnowledgeDelete({
  onDeleted,
  selectedItem,
}: UseKnowledgeDeleteOptions) {
  const { t } = useTranslation('common');
  const [isDeleting, setIsDeleting] = useState(false);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [prevSelectedItem, setPrevSelectedItem] = useState<KnowledgeChunkListItem | null>(selectedItem);

  if (selectedItem !== prevSelectedItem) {
    setPrevSelectedItem(selectedItem);
    if (!selectedItem) {
      setIsDeleteDialogOpen(false);
    }
  }

  const requestDelete = useCallback(() => {
    if (!selectedItem || isDeleting) {
      return;
    }

    setIsDeleteDialogOpen(true);
  }, [isDeleting, selectedItem]);

  const deleteSelectedItem = useCallback(async () => {
    if (!selectedItem || isDeleting) {
      return;
    }

    setIsDeleting(true);
    try {
      const response = await deleteGlobalKnowledge(selectedItem.id);
      toast.success(t('knowledge.deleteSuccess', 'Knowledge entry deleted.'), {
        description: t(
          'knowledge.deleteSuccessDescription',
          'Removed {{entities}} orphan entities and {{relationships}} orphan relationships.',
          {
            entities: response.orphanEntityCount,
            relationships: response.orphanRelationshipCount,
          },
        ),
      });
      setIsDeleteDialogOpen(false);
      onDeleted();
    } catch (error) {
      logger.error('Failed to delete knowledge entry', {
        error,
        id: selectedItem.id,
      });
      toast.error(
        t('knowledge.deleteFailed', 'Failed to delete knowledge entry.'),
      );
    } finally {
      setIsDeleting(false);
    }
  }, [isDeleting, onDeleted, selectedItem, t]);

  return {
    deleteSelectedItem,
    isDeleteDialogOpen,
    isDeleting,
    requestDelete,
    setIsDeleteDialogOpen,
  };
}
