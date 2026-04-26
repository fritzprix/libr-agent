import { useEffect, useState, type RefObject } from 'react';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import { useDnDContext } from '@/context/DnDContext';
import {
  checkDroppedPathType,
  registerDroppedFiles,
} from '@/lib/backend/file-operations';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useWorkspaceDropZone');

export function useWorkspaceDropZone(
  dropZoneRef: RefObject<HTMLElement>,
  onWorkspaceSelected: (path: string) => void,
) {
  const { t } = useTranslation();
  const { subscribe } = useDnDContext();
  const [workspaceDragState, setWorkspaceDragState] = useState<
    'none' | 'valid' | 'invalid'
  >('none');

  useEffect(() => {
    const processDroppedPaths = (paths: string[]) => {
      const run = async () => {
        try {
          await registerDroppedFiles(paths);
        } catch (error: unknown) {
          logger.error('Failed to register dropped workspace folder', error);
          toast.error(t('scheduledTasks.modal.workspaceRegisterFailed'));
          return;
        }

        for (const filePath of paths) {
          try {
            const pathType = await checkDroppedPathType(filePath);
            if (pathType === 'directory') {
              onWorkspaceSelected(filePath);
              return;
            }
          } catch (error: unknown) {
            logger.error(
              'Failed to inspect dropped workspace path',
              { filePath },
              error,
            );
          }
        }

        toast.error(t('scheduledTasks.modal.workspaceDropFolderError'));
      };

      void run();
    };

    const unsubscribe = subscribe(
      dropZoneRef,
      (event, payload) => {
        if (event === 'drag-over') {
          setWorkspaceDragState(
            payload.paths && payload.paths.length > 0 ? 'valid' : 'invalid',
          );
          return;
        }

        if (event === 'leave') {
          setWorkspaceDragState('none');
          return;
        }

        setWorkspaceDragState('none');
        if (payload.paths && payload.paths.length > 0) {
          processDroppedPaths(payload.paths);
        }
      },
      { priority: 5 },
    );

    return () => {
      unsubscribe();
    };
  }, [dropZoneRef, onWorkspaceSelected, subscribe, t]);

  return { workspaceDragState };
}
