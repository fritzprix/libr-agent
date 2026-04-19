import type { RefObject } from 'react';
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
import { formatSkillDisplayPath } from '@/features/settings/components/SkillsListModal';
import { cn } from '@/lib/utils';
import type {
  SkillsDirectoryScope,
  SkillsManagementDirectories,
  SkillsStatusMessage,
  SkillsVerificationStatus,
} from '../skills-management-types';

interface SkillsManagementPanelContentProps
  extends SkillsManagementDirectories {
  dropZoneRef: RefObject<HTMLDivElement>;
  isDragging: boolean;
  openingDirectory: SkillsDirectoryScope | null;
  repoUrl: string;
  verificationStatus: SkillsVerificationStatus;
  statusMessage: SkillsStatusMessage;
  isInstallingGithub: boolean;
  isInstallingLocal: boolean;
  isResettingUserSkills: boolean;
  onRepoUrlChange: (value: string) => void;
  onRefresh: () => void;
  onViewInstalled: () => void;
  onOpenDirectory: (
    directory: string,
    scope: SkillsDirectoryScope,
  ) => Promise<void> | void;
  onOpenResetDialog: () => void;
  onGitHubInstall: () => Promise<void> | void;
}

function StatusIcon({ kind }: Pick<SkillsStatusMessage, 'kind'>) {
  if (kind === 'loading') {
    return <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />;
  }

  if (kind === 'error') {
    return <AlertCircle className="w-4 h-4 text-destructive" />;
  }

  return <CheckCircle className="w-4 h-4 text-success" />;
}

export function SkillsManagementPanelContent({
  dropZoneRef,
  isDragging,
  openingDirectory,
  repoUrl,
  verificationStatus,
  statusMessage,
  systemDirectory,
  userDirectory,
  systemSkills,
  userSkills,
  isInstallingGithub,
  isInstallingLocal,
  isResettingUserSkills,
  onRepoUrlChange,
  onRefresh,
  onViewInstalled,
  onOpenDirectory,
  onOpenResetDialog,
  onGitHubInstall,
}: SkillsManagementPanelContentProps) {
  const { t } = useTranslation('common');

  return (
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
            onClick={onRefresh}
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
            onClick={onViewInstalled}
          >
            {t('settings.skills.viewInstalled', 'View installed skills')}
          </Button>
        </div>
      </div>

      <div className="flex items-center gap-2 text-sm">
        <StatusIcon kind={statusMessage.kind} />
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
                      void onOpenDirectory(systemDirectory, 'system')
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
                        void onOpenDirectory(userDirectory, 'user')
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
                onClick={onOpenResetDialog}
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
              onChange={(event) => onRepoUrlChange(event.target.value)}
              placeholder={t(
                'settings.skills.githubInstallPlaceholder',
                'https://github.com/owner/repo or tree URL',
              )}
            />
            <Button
              type="button"
              onClick={() => void onGitHubInstall()}
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
  );
}
