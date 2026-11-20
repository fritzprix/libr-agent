import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { Badge, Checkbox, ScrollArea } from '@/components/ui';
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
import { Button } from '@/components/ui/button';
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
  onApproveHooksChange: (approved: boolean) => void;
  onConfirm: () => Promise<void> | void;
  onConfirmWithoutHooks: () => Promise<void> | void;
}

export function SkillsConflictDialog({
  pendingInstall,
  userConflicts,
  systemConflicts,
  isInstallingLocal,
  isInstallingGithub,
  onOpenChange,
  onToggleOverwrite,
  onApproveHooksChange,
  onConfirm,
  onConfirmWithoutHooks,
}: SkillsConflictDialogProps) {
  const { t } = useTranslation('common');
  const hasHooks = (pendingInstall?.preview.hooks.length ?? 0) > 0;
  const discoveredSkills = pendingInstall?.preview.discoveredSkills ?? [];
  const title = hasHooks
    ? t('settings.skills.installConfirmTitle', 'Review skills and hooks')
    : t('settings.skills.conflictTitle', 'Replace conflicting skills?');

  return (
    <AlertDialog open={pendingInstall !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent className="grid max-h-[85vh] max-w-2xl grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0">
        <AlertDialogHeader className="px-6 pt-6 pb-4">
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>
            {hasHooks
              ? t(
                  'settings.skills.installConfirmDescription',
                  'Confirm which skills to install and whether plugin hooks may run on session start.',
                )
              : t(
                  'settings.skills.conflictDescription',
                  'Choose which existing user skills should be overwritten. Bundled system skill collisions will be skipped automatically.',
                )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="min-h-0 overflow-hidden px-6">
          <ScrollArea className="h-full pr-4">
            <div className="space-y-3 pb-1">
              {discoveredSkills.length > 0 ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t('settings.skills.skillsToInstall', 'Skills to install')}
                  </p>
                  {discoveredSkills.map((skill) => (
                    <div key={skill.name}>
                      <p className="font-medium">{skill.name}</p>
                      <p className="text-xs text-muted-foreground">
                        {skill.description}
                      </p>
                    </div>
                  ))}
                </div>
              ) : null}
              {hasHooks ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                  <p className="font-medium">
                    {t('settings.skills.hooksToInstall', 'Plugin hooks')}
                  </p>
                  {pendingInstall?.preview.hooks.map((hook, index) => (
                    <div
                      key={`${hook.event}-${hook.command}-${index}`}
                      className="min-w-0 space-y-1 border-t border-border/60 pt-2 first:border-t-0 first:pt-0"
                    >
                      <div className="flex min-w-0 flex-wrap items-center gap-2">
                        <span className="font-medium">{hook.event}</span>
                        {hook.matcher ? (
                          <span className="min-w-0 break-all text-xs text-muted-foreground">
                            {hook.matcher}
                          </span>
                        ) : null}
                        <Badge variant="secondary">
                          {hook.executionStatus === 'executed'
                            ? t('settings.skills.hookStatusExecuted', 'Executed')
                            : t(
                                'settings.skills.hookStatusStubbed',
                                'Not supported',
                              )}
                        </Badge>
                      </div>
                      <p className="text-xs text-muted-foreground break-all font-mono">
                        {hook.command}
                      </p>
                    </div>
                  ))}
                  <label className="flex items-start gap-3 cursor-pointer pt-2">
                    <Checkbox
                      checked={pendingInstall?.approveHooks ?? false}
                      onCheckedChange={(checked) =>
                        onApproveHooksChange(checked === true)
                      }
                      className="mt-0.5"
                    />
                    <div>
                      <p className="font-medium">
                        {t(
                          'settings.skills.approveHooksLabel',
                          'Allow hooks to run',
                        )}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {t(
                          'settings.skills.approveHooksHint',
                          'Supported hook events run automatically once approved. Unsupported events are skipped.',
                        )}
                      </p>
                    </div>
                  </label>
                </div>
              ) : null}
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
        <AlertDialogFooter className="min-w-0 px-6 pt-4 pb-6 flex-col sm:flex-row sm:flex-wrap sm:justify-end gap-2">
          <AlertDialogCancel disabled={isInstallingLocal || isInstallingGithub}>
            {t('common.cancel', 'Cancel')}
          </AlertDialogCancel>
          {hasHooks ? (
            <Button
              type="button"
              variant="outline"
              disabled={isInstallingLocal || isInstallingGithub}
              onClick={(event) => {
                event.preventDefault();
                void onConfirmWithoutHooks();
              }}
            >
              {(isInstallingLocal || isInstallingGithub) && (
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              )}
              {t(
                'settings.skills.installWithoutHooks',
                'Install without hook approval',
              )}
            </Button>
          ) : null}
          <AlertDialogAction
            disabled={
              isInstallingLocal ||
              isInstallingGithub ||
              (hasHooks && !pendingInstall?.approveHooks)
            }
            onClick={(event) => {
              event.preventDefault();
              void onConfirm();
            }}
          >
            {(isInstallingLocal || isInstallingGithub) && (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            )}
            {hasHooks
              ? t(
                  'settings.skills.installWithHooks',
                  'Install and approve hooks',
                )
              : userConflicts.length > 0 &&
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
