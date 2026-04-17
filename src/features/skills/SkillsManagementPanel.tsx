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
  Button,
  Input,
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
    (importedCount: number, overwrittenCount: number) => {
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

      return t('settings.skills.importSuccess', {
        count: importedCount,
        defaultValue_one: 'Imported {{count}} skill',
        defaultValue_other: 'Imported {{count}} skills',
      });
    },
    [t],
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
          if (preview.conflicts.length > 0) {
            setPendingInstall({
              kind: 'local',
              sourceValue: filePath,
              preview,
            });
            return;
          }

          setIsInstallingLocal(true);
          const result = await importUserSkills(filePath, false);
          toast.success(
            formatImportSuccess(
              result.importedNames.length,
              result.overwrittenNames.length,
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
      if (preview.conflicts.length > 0) {
        setPendingInstall({
          kind: 'github',
          sourceValue: trimmedRepoUrl,
          preview,
        });
        return;
      }

      const result = await installGitHubSkills(trimmedRepoUrl, false);
      toast.success(
        formatImportSuccess(
          result.importedNames.length,
          result.overwrittenNames.length,
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
      const result =
        kind === 'local'
          ? await importUserSkills(sourceValue, true)
          : await installGitHubSkills(sourceValue, true);
      toast.success(
        formatImportSuccess(
          result.importedNames.length,
          result.overwrittenNames.length,
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
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.skills.conflictTitle',
                'Replace conflicting skills?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'settings.skills.conflictDescription',
                'The following skill names already exist. Replacing them will update the managed user copy or shadow the bundled system skill.',
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
            {pendingInstall?.preview.conflicts.map((conflict) => (
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
                  {conflict.existingOrigin}
                </span>
              </div>
            ))}
          </div>
          <AlertDialogFooter>
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
              {t('settings.skills.replaceConflicts', 'Replace and install')}
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
