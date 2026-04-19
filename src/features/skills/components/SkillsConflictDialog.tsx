import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { Checkbox, ScrollArea } from '@/components/ui';
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
import type { SkillImportConflict } from '@/types/skills';
import type { PendingInstall } from '../skills-management-types';

interface SkillsConflictDialogProps {
  pendingInstall: PendingInstall | null;
  userConflicts: SkillImportConflict[];
  systemConflicts: SkillImportConflict[];
  isInstallingLocal: boolean;
  isInstallingGithub: boolean;
  onOpenChange: (open: boolean) => void;
  onToggleOverwrite: (skillName: string) => void;
  onConfirm: () => Promise<void> | void;
}

export function SkillsConflictDialog({
  pendingInstall,
  userConflicts,
  systemConflicts,
  isInstallingLocal,
  isInstallingGithub,
  onOpenChange,
  onToggleOverwrite,
  onConfirm,
}: SkillsConflictDialogProps) {
  const { t } = useTranslation('common');

  return (
    <AlertDialog open={pendingInstall !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent className="grid max-h-[85vh] max-w-2xl grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0">
        <AlertDialogHeader className="px-6 pt-6 pb-4">
          <AlertDialogTitle>
            {t('settings.skills.conflictTitle', 'Replace conflicting skills?')}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {t(
              'settings.skills.conflictDescription',
              'Choose which existing user skills should be overwritten. Bundled system skill collisions will be skipped automatically.',
            )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="min-h-0 overflow-hidden px-6">
          <ScrollArea className="h-full pr-4">
            <div className="space-y-3 pb-1">
              {userConflicts.length > 0 && (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                  {userConflicts.map((conflict) => {
                    const checked =
                      pendingInstall?.selectedOverwriteNames.includes(
                        conflict.name,
                      ) ?? false;

                    return (
                      <label
                        key={`${conflict.name}-${conflict.existingPath}`}
                        className="flex items-start gap-3 cursor-pointer"
                      >
                        <Checkbox
                          checked={checked}
                          onCheckedChange={() =>
                            onToggleOverwrite(conflict.name)
                          }
                          className="mt-0.5"
                        />
                        <div className="min-w-0">
                          <p className="font-medium">{conflict.name}</p>
                          <p className="text-xs text-muted-foreground">
                            {t(
                              'settings.skills.userConflictWillOverwrite',
                              'Checked items will overwrite the existing user skill.',
                            )}
                          </p>
                          <p className="text-xs text-muted-foreground break-all">
                            {conflict.existingPath}
                          </p>
                        </div>
                      </label>
                    );
                  })}
                </div>
              )}
              {systemConflicts.length > 0 && (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t(
                      'settings.skills.systemConflictsSkippedTitle',
                      'Bundled skills that will be skipped',
                    )}
                  </p>
                  {systemConflicts.map((conflict) => (
                    <div
                      key={`${conflict.name}-${conflict.existingPath}`}
                      className="flex items-start justify-between gap-3"
                    >
                      <div>
                        <p className="font-medium">{conflict.name}</p>
                        <p className="text-xs text-muted-foreground break-all">
                          {conflict.existingPath}
                        </p>
                      </div>
                      <span className="text-xs uppercase tracking-wide text-muted-foreground">
                        {t('settings.skills.skipped', 'Skipped')}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </ScrollArea>
        </div>
        <AlertDialogFooter className="px-6 pt-4 pb-6">
          <AlertDialogCancel disabled={isInstallingLocal || isInstallingGithub}>
            {t('common.cancel', 'Cancel')}
          </AlertDialogCancel>
          <AlertDialogAction
            disabled={isInstallingLocal || isInstallingGithub}
            onClick={(event) => {
              event.preventDefault();
              void onConfirm();
            }}
          >
            {(isInstallingLocal || isInstallingGithub) && (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            )}
            {userConflicts.length > 0 &&
            (pendingInstall?.selectedOverwriteNames.length ?? 0) > 0
              ? t('settings.skills.replaceConflicts', 'Replace and install')
              : t(
                  'settings.skills.installNonConflicting',
                  'Install remaining skills',
                )}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
