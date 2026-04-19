import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';

interface SkillsResetDialogProps {
  open: boolean;
  isResettingUserSkills: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void> | void;
}

export function SkillsResetDialog({
  open,
  isResettingUserSkills,
  onOpenChange,
  onConfirm,
}: SkillsResetDialogProps) {
  const { t } = useTranslation('common');

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t('settings.skills.resetUserSkillsTitle', 'Reset user skills?')}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {t(
              'settings.skills.resetUserSkillsDescription',
              'This removes every user-installed global skill while leaving bundled system skills untouched.',
            )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={isResettingUserSkills}>
            {t('common.cancel', 'Cancel')}
          </AlertDialogCancel>
          <AlertDialogAction
            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            disabled={isResettingUserSkills}
            onClick={(event) => {
              event.preventDefault();
              void onConfirm();
            }}
          >
            {isResettingUserSkills && (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            )}
            {t('settings.skills.resetUserSkills', 'Reset user skills')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
