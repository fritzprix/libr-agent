import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertCircle,
  CheckCircle,
  Download,
  FolderOutput,
  Github,
  Loader2,
  RefreshCw,
  Trash2,
  Upload,
} from 'lucide-react';
import {
  Checkbox,
  Button,
  Input,
  ScrollArea,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
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
import { useDnDContext } from '@/context/DnDContext';
import { safeInvoke } from '@/lib/backend/core';
import {
  deleteUserSkill,
  importUserSkills,
  installGitHubSkills,
  previewGitHubSkillInstall,
  previewUserSkillImport,
  resetUserSkills,
} from '@/lib/backend/skills';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { toast } from 'sonner';
import type { SkillImportPreview } from '@/types/skills';
import {
  formatSkillDisplayPath,
  SkillsListModal,
} from '@/features/settings/components/SkillsListModal';
import { useSkillsDirectory } from '@/features/settings/hooks/useSkillsDirectory';

const logger = getLogger('SkillsManagementPanel');

interface PendingInstall {
  kind: 'local' | 'github';
  sourceValue: string;
  preview: SkillImportPreview;
  selectedOverwriteNames: string[];
}

interface SkillsManagementPanelProps {
  className?: string;
}

function SkillsManagementPanelComponent({
  className,
}: SkillsManagementPanelProps) {
  const { t } = useTranslation('common');
  const { subscribe } = useDnDContext();
  const {
    verificationStatus,
    skills,
    systemSkills,
    userSkills,
    systemDirectory,
    userDirectory,
    errorMessage,
    refresh,
  } = useSkillsDirectory();

  const dropZoneRef = useRef<HTMLDivElement>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [pendingInstall, setPendingInstall] = useState<PendingInstall | null>(
    null,
  );
  const [isResetDialogOpen, setIsResetDialogOpen] = useState(false);
  const [isInstallingLocal, setIsInstallingLocal] = useState(false);
  const [isInstallingGithub, setIsInstallingGithub] = useState(false);
  const [isDeletingSkill, setIsDeletingSkill] = useState<string | null>(null);
  const [isResettingUserSkills, setIsResettingUserSkills] = useState(false);
  const [openingDirectory, setOpeningDirectory] = useState<
    'system' | 'user' | null
  >(null);
  const [repoUrl, setRepoUrl] = useState('');

  const formatImportSuccess = useCallback(
    (importedCount: number, overwrittenCount: number, skippedCount: number) => {
      if (importedCount === 0 && skippedCount > 0) {
        return t('settings.skills.importSkippedOnly', {
          count: skippedCount,
          defaultValue_one: 'Skipped {{count}} conflicting skill',
          defaultValue_other: 'Skipped {{count}} conflicting skills',
        });
      }

      if (overwrittenCount > 0 && skippedCount > 0) {
        return t('settings.skills.importSuccessWithOverwriteAndSkipped', {
          count: importedCount,
          overwrittenCount,
          skippedCount,
          defaultValue_one:
            'Imported {{count}} skill ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
          defaultValue_other:
            'Imported {{count}} skills ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
        });
      }

      if (overwrittenCount > 0) {
        return t('settings.skills.importSuccessWithOverwrite', {
          count: importedCount,
          overwrittenCount,
          defaultValue_one:
            'Imported {{count}} skill ({{overwrittenCount}} overwritten)',
          defaultValue_other:
            'Imported {{count}} skills ({{overwrittenCount}} overwritten)',
        });
      }

      if (skippedCount > 0) {
        return t('settings.skills.importSuccessWithSkipped', {
          count: importedCount,
          skippedCount,
          defaultValue_one:
            'Imported {{count}} skill ({{skippedCount}} skipped)',
          defaultValue_other:
            'Imported {{count}} skills ({{skippedCount}} skipped)',
        });
      }

      return t('settings.skills.importSuccess', {
        count: importedCount,
        defaultValue_one: 'Imported {{count}} skill',
        defaultValue_other: 'Imported {{count}} skills',
      });
    },
    [t],
  );

  const pendingInstallSystemConflicts = useMemo(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'system',
      ) ?? [],
    [pendingInstall],
  );
  const pendingInstallUserConflicts = useMemo(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'user',
      ) ?? [],
    [pendingInstall],
  );

  const performInstall = useCallback(
    async (
      kind: PendingInstall['kind'],
      sourceValue: string,
      overwriteNames: string[],
      extraExcludedNames: string[] = [],
    ) => {
      const userConflictNames = new Set(
        pendingInstallUserConflicts.map((conflict) => conflict.name),
      );
      const excludedNames = [
        ...pendingInstallSystemConflicts.map((conflict) => conflict.name),
        ...pendingInstallUserConflicts
          .map((conflict) => conflict.name)
          .filter((name) => !overwriteNames.includes(name)),
        ...extraExcludedNames,
      ].filter((name, index, names) => names.indexOf(name) === index);
      const shouldOverwrite = overwriteNames.some((name) =>
        userConflictNames.has(name),
      );

      return kind === 'local'
        ? importUserSkills(sourceValue, shouldOverwrite, excludedNames)
        : installGitHubSkills(sourceValue, shouldOverwrite, excludedNames);
    },
    [pendingInstallSystemConflicts, pendingInstallUserConflicts],
  );

  useEffect(() => {
    if (!dropZoneRef.current) {
      return;
    }

    return subscribe(
      dropZoneRef,
      async (event, payload) => {
        if (event === 'drag-over') {
          if (payload.paths && payload.paths.length > 0) {
            setIsDragging(true);
          }
          return;
        }

        if (event === 'leave') {
          setIsDragging(false);
          return;
        }

        setIsDragging(false);
        const filePath = payload.paths?.[0];
        if (!filePath || isInstallingLocal || isInstallingGithub) {
          return;
        }

        const toastId = toast.loading(
          t(
            'settings.skills.importingUserSkills',
            'Inspecting dropped skill package...',
          ),
        );

        try {
          const preview = await previewUserSkillImport(filePath);
          toast.dismiss(toastId);
          const userConflicts = preview.conflicts.filter(
            (conflict) => conflict.existingOrigin === 'user',
          );
          if (userConflicts.length > 0) {
            setPendingInstall({
              kind: 'local',
              sourceValue: filePath,
              preview,
              selectedOverwriteNames: userConflicts.map(
                (conflict) => conflict.name,
              ),
            });
            return;
          }

          setIsInstallingLocal(true);
          const result = await importUserSkills(
            filePath,
            false,
            preview.conflicts
              .filter((conflict) => conflict.existingOrigin === 'system')
              .map((conflict) => conflict.name),
          );
          toast.success(
            formatImportSuccess(
              result.importedNames.length,
              result.overwrittenNames.length,
              result.skippedNames.length,
            ),
          );
          await refresh();
        } catch (error) {
          logger.error('Failed to import dropped skills', error);
          toast.error(error instanceof Error ? error.message : String(error), {
            id: toastId,
          });
        } finally {
          setIsInstallingLocal(false);
        }
      },
      { priority: 2 },
    );
  }, [
    formatImportSuccess,
    isInstallingGithub,
    isInstallingLocal,
    refresh,
    subscribe,
    t,
  ]);

  const handleOpenDirectory = async (
    directory: string,
    scope: 'system' | 'user',
  ) => {
    if (!directory || openingDirectory) {
      return;
    }

    setOpeningDirectory(scope);
    try {
      await safeInvoke<void>('open_skills_directory_in_explorer', {
        directory,
      });
    } catch (error) {
      logger.error(`Failed to open ${scope} skills directory`, error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setOpeningDirectory(null);
    }
  };

  const handleGitHubInstall = async () => {
    const trimmedRepoUrl = repoUrl.trim();
    if (!trimmedRepoUrl || isInstallingGithub || isInstallingLocal) {
      return;
    }

    setIsInstallingGithub(true);
    const toastId = toast.loading(
      t('settings.skills.githubInspecting', 'Inspecting GitHub repository...'),
    );

    try {
      const preview = await previewGitHubSkillInstall(trimmedRepoUrl);
      toast.dismiss(toastId);
      const userConflicts = preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'user',
      );
      if (userConflicts.length > 0) {
        setPendingInstall({
          kind: 'github',
          sourceValue: trimmedRepoUrl,
          preview,
          selectedOverwriteNames: userConflicts.map(
            (conflict) => conflict.name,
          ),
        });
        return;
      }

      const result = await installGitHubSkills(
        trimmedRepoUrl,
        false,
        preview.conflicts
          .filter((conflict) => conflict.existingOrigin === 'system')
          .map((conflict) => conflict.name),
      );
      toast.success(
        formatImportSuccess(
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      setRepoUrl('');
      await refresh();
    } catch (error) {
      logger.error('Failed to install skills from GitHub', error);
      toast.error(error instanceof Error ? error.message : String(error), {
        id: toastId,
      });
    } finally {
      setIsInstallingGithub(false);
    }
  };

  const confirmPendingInstall = async () => {
    if (!pendingInstall) {
      return;
    }

    const { kind, sourceValue } = pendingInstall;
    setIsInstallingLocal(kind === 'local');
    setIsInstallingGithub(kind === 'github');

    try {
      const result = await performInstall(
        kind,
        sourceValue,
        pendingInstall.selectedOverwriteNames,
      );
      toast.success(
        formatImportSuccess(
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      if (kind === 'github') {
        setRepoUrl('');
      }
      setPendingInstall(null);
      await refresh();
    } catch (error) {
      logger.error('Failed to confirm skill install overwrite', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsInstallingLocal(false);
      setIsInstallingGithub(false);
    }
  };

  const togglePendingOverwrite = useCallback((skillName: string) => {
    setPendingInstall((current) => {
      if (!current) {
        return current;
      }

      return {
        ...current,
        selectedOverwriteNames: current.selectedOverwriteNames.includes(
          skillName,
        )
          ? current.selectedOverwriteNames.filter((name) => name !== skillName)
          : [...current.selectedOverwriteNames, skillName],
      };
    });
  }, []);

  const handleDeleteUserSkill = async (skillName: string) => {
    if (isDeletingSkill) {
      return;
    }

    setIsDeletingSkill(skillName);
    try {
      await deleteUserSkill(skillName);
      toast.success(
        t('settings.skills.userSkillDeleted', 'User skill deleted.'),
      );
      await refresh();
    } catch (error) {
      logger.error('Failed to delete user skill', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsDeletingSkill(null);
    }
  };

  const handleResetUserSkills = async () => {
    if (isResettingUserSkills) {
      return;
    }

    setIsResettingUserSkills(true);
    try {
      await resetUserSkills();
      toast.success(t('settings.skills.userSkillsReset', 'User skills reset.'));
      setIsResetDialogOpen(false);
      await refresh();
    } catch (error) {
      logger.error('Failed to reset user skills', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsResettingUserSkills(false);
    }
  };

  const statusMessage = useMemo(() => {
    if (verificationStatus === 'loading') {
      return {
        icon: (
          <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
        ),
        text: t('settings.general.verifying', 'Verifying...'),
        tone: 'text-muted-foreground',
      };
    }

    if (verificationStatus === 'error') {
      return {
        icon: <AlertCircle className="w-4 h-4 text-destructive" />,
        text:
          errorMessage ||
          t('settings.general.invalidDirectory', 'Invalid directory'),
        tone: 'text-destructive',
      };
    }

    return {
      icon: <CheckCircle className="w-4 h-4 text-success" />,
      text: t('settings.skills.installedCount', {
        count: skills.length,
        defaultValue: '{{count}} installed skills available',
      }),
      tone: 'text-success',
    };
  }, [errorMessage, skills.length, t, verificationStatus]);

  return (
    <div className={cn('space-y-6', className)}>
      <div
        ref={dropZoneRef}
        className={cn(
          'relative rounded-xl border border-border/70 p-4 space-y-4 transition-colors',
          isDragging && 'border-primary ring-2 ring-primary/20 bg-primary/5',
        )}
      >
        {isDragging && (
          <div className="absolute inset-0 z-10 rounded-xl border-2 border-dashed border-primary bg-background/80 flex items-center justify-center">
            <div className="flex flex-col items-center gap-2 text-center">
              <Upload className="w-10 h-10 text-primary" />
              <p className="text-sm font-medium">
                {t(
                  'settings.skills.dropZoneTitle',
                  'Drop a .skill, .zip, or skill folder to install',
                )}
              </p>
            </div>
          </div>
        )}

        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <h4 className="font-medium text-foreground">
              {t('settings.skills.managerTitle', 'Managed Skills Library')}
            </h4>
            <p className="text-sm text-muted-foreground max-w-2xl">
              {t(
                'settings.skills.managerDescription',
                'Install skills into LibrAgent-managed storage. System skills stay read-only, while user skills can be added, replaced, or reset safely.',
              )}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void refresh()}
              disabled={verificationStatus === 'loading'}
            >
              <RefreshCw
                className={cn(
                  'w-4 h-4 mr-2',
                  verificationStatus === 'loading' && 'animate-spin',
                )}
              />
              {t('common.refresh', 'Refresh')}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setIsModalOpen(true)}
            >
              {t('settings.skills.viewInstalled', 'View installed skills')}
            </Button>
          </div>
        </div>

        <div className="flex items-center gap-2 text-sm">
          {statusMessage.icon}
          <span className={statusMessage.tone}>{statusMessage.text}</span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="rounded-lg border bg-muted/20 p-3 space-y-2">
            <div className="flex items-center justify-between gap-2">
              <div>
                <p className="text-sm font-medium">
                  {t(
                    'settings.skills.systemDirectoryTitle',
                    'System Skills Directory',
                  )}
                </p>
                <p className="text-xs text-muted-foreground break-all">
                  {formatSkillDisplayPath(systemDirectory)}
                </p>
              </div>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={openingDirectory !== null}
                      onClick={() =>
                        void handleOpenDirectory(systemDirectory, 'system')
                      }
                    >
                      <FolderOutput className="w-4 h-4" />
                    </Button>
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  {t(
                    'settings.skills.openSystemDirectory',
                    'Open system skills directory',
                  )}
                </TooltipContent>
              </Tooltip>
            </div>
            <p className="text-xs text-muted-foreground">
              {t('settings.skills.systemCount', {
                count: systemSkills.length,
                defaultValue: '{{count}} bundled read-only system skills',
              })}
            </p>
          </div>

          <div className="rounded-lg border bg-muted/20 p-3 space-y-2">
            <div className="flex items-center justify-between gap-2">
              <div>
                <p className="text-sm font-medium">
                  {t(
                    'settings.skills.userDirectoryTitle',
                    'User Skills Directory',
                  )}
                </p>
                <p className="text-xs text-muted-foreground break-all">
                  {formatSkillDisplayPath(userDirectory)}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={openingDirectory !== null}
                        onClick={() =>
                          void handleOpenDirectory(userDirectory, 'user')
                        }
                      >
                        <FolderOutput className="w-4 h-4" />
                      </Button>
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t(
                      'settings.skills.openUserDirectory',
                      'Open user skills directory',
                    )}
                  </TooltipContent>
                </Tooltip>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={userSkills.length === 0 || isResettingUserSkills}
                  onClick={() => setIsResetDialogOpen(true)}
                >
                  <Trash2 className="w-4 h-4 mr-2" />
                  {t('settings.skills.resetUserSkills', 'Reset user skills')}
                </Button>
              </div>
            </div>
            <p className="text-xs text-muted-foreground">
              {t('settings.skills.userCount', {
                count: userSkills.length,
                defaultValue: '{{count}} user-installed global skills',
              })}
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <div className="rounded-lg border bg-background p-3 space-y-2">
            <div className="flex items-center gap-2">
              <Upload className="w-4 h-4 text-primary" />
              <p className="text-sm font-medium">
                {t('settings.skills.localInstallTitle', 'Local install')}
              </p>
            </div>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.skills.localInstallDescription',
                'Drag and drop a .skill file, .zip archive, or skill folder anywhere in this panel to import it into managed user storage.',
              )}
            </p>
          </div>

          <div className="rounded-lg border bg-background p-3 space-y-3">
            <div className="flex items-center gap-2">
              <Github className="w-4 h-4 text-primary" />
              <p className="text-sm font-medium">
                {t(
                  'settings.skills.githubInstallTitle',
                  'Install from GitHub repository',
                )}
              </p>
            </div>
            <div className="flex flex-col gap-2 md:flex-row">
              <Input
                value={repoUrl}
                onChange={(event) => setRepoUrl(event.target.value)}
                placeholder={t(
                  'settings.skills.githubInstallPlaceholder',
                  'https://github.com/owner/repo or tree URL',
                )}
              />
              <Button
                type="button"
                onClick={() => void handleGitHubInstall()}
                disabled={
                  isInstallingGithub || isInstallingLocal || !repoUrl.trim()
                }
              >
                {isInstallingGithub ? (
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                ) : (
                  <Download className="w-4 h-4 mr-2" />
                )}
                {t('settings.skills.githubInstallButton', 'Install')}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {t(
                'settings.skills.githubInstallDescription',
                'Public GitHub repositories are downloaded, scanned for SKILL.md packages, and then installed into managed user storage.',
              )}
            </p>
          </div>
        </div>
      </div>

      <SkillsListModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        systemSkills={systemSkills}
        userSkills={userSkills}
        deletingSkillName={isDeletingSkill}
        onDeleteUserSkill={(skillName) => void handleDeleteUserSkill(skillName)}
      />

      <AlertDialog
        open={pendingInstall !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPendingInstall(null);
          }
        }}
      >
        <AlertDialogContent className="grid max-h-[85vh] max-w-2xl grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0">
          <AlertDialogHeader className="px-6 pt-6 pb-4">
            <AlertDialogTitle>
              {t(
                'settings.skills.conflictTitle',
                'Replace conflicting skills?',
              )}
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
                {pendingInstallUserConflicts.length > 0 && (
                  <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                    {pendingInstallUserConflicts.map((conflict) => {
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
                              togglePendingOverwrite(conflict.name)
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
                {pendingInstallSystemConflicts.length > 0 && (
                  <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                    <p className="font-medium">
                      {t(
                        'settings.skills.systemConflictsSkippedTitle',
                        'Bundled skills that will be skipped',
                      )}
                    </p>
                    {pendingInstallSystemConflicts.map((conflict) => (
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
            <AlertDialogCancel
              disabled={isInstallingLocal || isInstallingGithub}
            >
              {t('common.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={isInstallingLocal || isInstallingGithub}
              onClick={(event) => {
                event.preventDefault();
                void confirmPendingInstall();
              }}
            >
              {(isInstallingLocal || isInstallingGithub) && (
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              )}
              {pendingInstallUserConflicts.length > 0 &&
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

      <AlertDialog
        open={isResetDialogOpen}
        onOpenChange={(open) => {
          if (!isResettingUserSkills) {
            setIsResetDialogOpen(open);
          }
        }}
      >
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
                void handleResetUserSkills();
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
    </div>
  );
}

export const SkillsManagementPanel = React.memo(SkillsManagementPanelComponent);
