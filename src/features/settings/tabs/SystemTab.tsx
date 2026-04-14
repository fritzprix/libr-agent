import React, { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { SystemSettings } from '@/context/SettingsContext';
import { SystemPerformanceSettings } from '../components/SystemPerformanceSettings';
import {
  FolderOpen,
  FolderOutput,
  Loader2,
  CheckCircle,
  AlertCircle,
} from 'lucide-react';
import {
  Button,
  Input,
  Slider,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { cn } from '@/lib/utils';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { getLogger } from '@/lib/logger';
import { useSkillsDirectory } from '../hooks/useSkillsDirectory';
import { safeInvoke } from '@/lib/backend/core';
import { SkillsListModal } from '../components/SkillsListModal';

const logger = getLogger('SystemTab');

interface SystemTabProps {
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    networkSettingsChanged: boolean;
    onChange: (
      key: keyof SystemSettings,
      value: string | number | boolean,
    ) => void;
  };
}

const STORAGE_PRESETS_MB = [10, 25, 50, 100, 250, 500] as const;

function findNearestStorageIndex(value: number): number {
  return STORAGE_PRESETS_MB.reduce((bestIndex, preset, index) => {
    const bestDistance = Math.abs(STORAGE_PRESETS_MB[bestIndex] - value);
    const nextDistance = Math.abs(preset - value);
    return nextDistance < bestDistance ? index : bestIndex;
  }, 0);
}

function SystemTabComponent({ systemSettingsProps }: SystemTabProps) {
  const { t } = useTranslation('common');
  const { localSystemSettings, onChange } = systemSettingsProps;
  const { effectiveDir, verificationStatus, skills, errorMessage } =
    useSkillsDirectory(localSystemSettings.skillsDirectory);

  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isOpeningDir, setIsOpeningDir] = useState(false);
  const [isBrowsing, setIsBrowsing] = useState(false);
  const openingDirLock = useRef(false);

  const handleBrowseEvents = async () => {
    if (isBrowsing) return;
    setIsBrowsing(true);
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: t(
          'settings.general.skillsDirectorySelectTitle',
          'Select Skills Directory',
        ),
      });

      if (selected && typeof selected === 'string') {
        onChange('skillsDirectory', selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
    } finally {
      setIsBrowsing(false);
    }
  };

  const handleOpenDirectory = async () => {
    if (!effectiveDir || isOpeningDir || openingDirLock.current) {
      return;
    }
    openingDirLock.current = true;
    setIsOpeningDir(true);
    try {
      await safeInvoke<void>('open_skills_directory_in_explorer', {
        directory: effectiveDir,
      });
    } catch (error) {
      logger.error('Failed to open directory', error);
    } finally {
      setIsOpeningDir(false);
      openingDirLock.current = false;
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.tabs.system', 'System')}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          {t(
            'settings.system.description',
            'Control app runtime, background workers, automation limits, and network behavior.',
          )}
        </p>
      </div>

      <div className="border-t pt-6">
        <h3 className="mb-4 text-lg font-medium text-foreground">
          {t('settings.system.fileWorkspace', 'File & Workspace')}
        </h3>

        <div className="space-y-6">
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t('settings.general.skillsDirectory', 'Skills Directory')}
            </label>
            <div className="flex flex-col gap-2">
              <div className="flex gap-2 max-w-lg">
                <Input
                  value={localSystemSettings.skillsDirectory ?? ''}
                  onChange={(e) => onChange('skillsDirectory', e.target.value)}
                  placeholder={
                    effectiveDir ||
                    t(
                      'settings.general.skillsDirectoryPlaceholder',
                      'Select a directory for local skills...',
                    )
                  }
                  className="bg-background border text-foreground flex-1"
                />
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span
                      className={cn(
                        'inline-block',
                        isBrowsing && 'cursor-not-allowed',
                      )}
                    >
                      <Button
                        variant="outline"
                        onClick={handleBrowseEvents}
                        className="px-3"
                        disabled={isBrowsing}
                        aria-label={t(
                          'settings.general.browseAriaLabel',
                          'Browse skills directory',
                        )}
                      >
                        {isBrowsing ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <FolderOpen className="w-4 h-4" />
                        )}
                      </Button>
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t('settings.general.browse', 'Browse')}
                  </TooltipContent>
                </Tooltip>

                {effectiveDir && (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span
                        className={cn(
                          'inline-block',
                          isOpeningDir && 'cursor-not-allowed',
                        )}
                      >
                        <Button
                          variant="outline"
                          onClick={handleOpenDirectory}
                          className="px-3"
                          disabled={isOpeningDir}
                          aria-label={t(
                            'settings.general.openInExplorer',
                            'Open in Explorer',
                          )}
                        >
                          <FolderOutput className="w-4 h-4" />
                        </Button>
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('settings.general.openInExplorer', 'Open in Explorer')}
                    </TooltipContent>
                  </Tooltip>
                )}
              </div>

              <div className="flex items-center gap-2 text-sm h-5">
                {verificationStatus === 'loading' && (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                    <span className="text-muted-foreground">
                      {t('settings.general.verifying', 'Verifying...')}
                    </span>
                  </>
                )}
                {verificationStatus === 'success' && (
                  <button
                    onClick={() => setIsModalOpen(true)}
                    className="flex items-center gap-2 hover:underline rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                  >
                    <CheckCircle className="w-4 h-4 text-success" />
                    <span className="text-success">
                      {t('settings.general.skillsFound', {
                        count: skills.length,
                      })}
                    </span>
                  </button>
                )}
                {verificationStatus === 'error' && (
                  <>
                    <AlertCircle className="w-4 h-4 text-destructive" />
                    <span className="text-destructive">
                      {errorMessage
                        ? t('settings.general.error', { message: errorMessage })
                        : t(
                            'settings.general.invalidDirectory',
                            'Invalid directory',
                          )}
                    </span>
                  </>
                )}
              </div>
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.general.skillsDirectoryDescription',
                'Directory containing Agent Skills (folders with SKILL.md). New skills will be discovered automatically.',
              )}
            </p>
          </div>

          <div className="min-w-0 rounded-xl border border-border/70 p-4 max-w-lg">
            <div className="mb-4 flex items-center justify-between gap-3">
              <label className="block text-muted-foreground font-medium">
                {t(
                  'settings.system.maxFileUploadSize',
                  'Max File Upload Size (MB)',
                )}
              </label>
              <span className="rounded-md bg-primary/10 px-2 py-1 text-sm font-mono text-primary">
                {`${localSystemSettings.maxFileUploadSizeMB} MB`}
              </span>
            </div>
            <Slider
              min={0}
              max={STORAGE_PRESETS_MB.length - 1}
              step={1}
              value={[
                findNearestStorageIndex(
                  localSystemSettings.maxFileUploadSizeMB,
                ),
              ]}
              onValueChange={([index]) =>
                onChange('maxFileUploadSizeMB', STORAGE_PRESETS_MB[index] ?? 50)
              }
            />
            <div className="mt-3 flex flex-wrap gap-2">
              {STORAGE_PRESETS_MB.map((preset) => (
                <Button
                  key={preset}
                  type="button"
                  variant={
                    preset === localSystemSettings.maxFileUploadSizeMB
                      ? 'default'
                      : 'outline'
                  }
                  className="h-8 px-2 text-xs"
                  onClick={() => onChange('maxFileUploadSizeMB', preset)}
                >
                  {`${preset} MB`}
                </Button>
              ))}
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t(
                'settings.system.maxFileUploadSizeDescription',
                'Maximum size for a single file upload. Increase if you often work with large documents.',
              )}
            </p>
          </div>
        </div>
      </div>

      <div className="border-t pt-6">
        <SystemPerformanceSettings {...systemSettingsProps} />
      </div>

      <SkillsListModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        skills={skills}
      />
    </div>
  );
}

export default React.memo(SystemTabComponent, (prev, next) => {
  return (
    prev.systemSettingsProps.networkSettingsChanged ===
      next.systemSettingsProps.networkSettingsChanged &&
    prev.systemSettingsProps.localSystemSettings ===
      next.systemSettingsProps.localSystemSettings &&
    prev.systemSettingsProps.onChange === next.systemSettingsProps.onChange
  );
});
