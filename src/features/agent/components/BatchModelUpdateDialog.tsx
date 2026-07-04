import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { useTranslation } from 'react-i18next';

interface BatchModelUpdateDialogProps {
  isOpen: boolean;
  onClose: () => void;
  parentSessionName: string;
  subSessionCount: number;
  newModel: string;
  newProvider: string;
  onConfirm: (recursive: boolean) => void;
}

export function BatchModelUpdateDialog({
  isOpen,
  onClose,
  parentSessionName,
  subSessionCount,
  newModel,
  newProvider,
  onConfirm,
}: BatchModelUpdateDialogProps) {
  const { t } = useTranslation();

  return (
    <AlertDialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <AlertDialogContent className="sm:max-w-[480px]">
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t('agent.statusBar.batchModelUpdate.title')}
          </AlertDialogTitle>
          <AlertDialogDescription className="space-y-3 pt-2 text-sm leading-relaxed text-muted-foreground">
            {t('agent.statusBar.batchModelUpdate.description', {
              parentSessionName,
              subSessionCount,
              newModel,
              newProvider,
            })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end sm:gap-0">
          <Button variant="outline" onClick={onClose} className="sm:mr-2">
            {t('agent.statusBar.batchModelUpdate.cancel')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onConfirm(false);
              onClose();
            }}
            className="sm:mr-2"
          >
            {t('agent.statusBar.batchModelUpdate.onlyThis')}
          </Button>
          <Button
            variant="default"
            onClick={() => {
              onConfirm(true);
              onClose();
            }}
          >
            {t('agent.statusBar.batchModelUpdate.all')}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
