import { safeInvoke } from '@/lib/backend/core';
import { useState, useRef } from 'react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Button, Input } from '@/components/ui';
import {
  FolderOpen,
  FolderOutput,
  Loader2,
  CheckCircle,
  AlertCircle,
} from 'lucide-react';
import { SkillsListModal } from '../components/SkillsListModal';
import { getLogger } from '@/lib/logger';
import { useSkillsDirectory } from '../hooks/useSkillsDirectory';

const logger = getLogger('GeneralTab');

interface GeneralTabProps {
  localLanguage: string;
  onChange: (lang: string) => void;
  skillsDirectory?: string;
  onSkillsDirectoryChange: (path: string) => void;
}

function GeneralTabComponent({
  localLanguage,
  onChange,
  skillsDirectory,
  onSkillsDirectoryChange,
}: GeneralTabProps) {
  const { t } = useTranslation('common');
  const { effectiveDir, verificationStatus, skills, errorMessage } = useSkillsDirectory(
    skillsDirectory,
  );
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
        onSkillsDirectoryChange(selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
    } finally {
      setIsBrowsing(false);
    }
  };

  const handleOpenDirectory = async () => {
    if (!effectiveDir || isOpeningDir || openingDirLock.current) {
      logger.warn(
        'handleOpenDirectory called but effectiveDir is empty or already opening',
      );
      return;
    }
    openingDirLock.current = true;
    setIsOpeningDir(true);
    logger.info(`Attempting to open directory: ${effectiveDir}`);
    try {
      await safeInvoke<void>('open_skills_directory_in_explorer', {
        directory: effectiveDir,
      });
      logger.info(`Successfully requested open_skills_directory_in_explorer`);
    } catch (error) {
      logger.error('Failed to open directory', error);
      // Optional: show toast error
    } finally {
      setIsOpeningDir(false);
      openingDirLock.current = false;
    }
  };

  return (
    <div className="space-y-6">
      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.language.label', 'Language')}
        </label>
        <select
          className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
          value={localLanguage}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="en">{t('settings.language.en', 'English')}</option>
          <option value="ko">{t('settings.language.ko', '한국어')}</option>
          <option value="zh">{t('settings.language.zh', '简体中文')}</option>
          <option value="ja">{t('settings.language.ja', '日本語')}</option>
          <option value="fr">{t('settings.language.fr', 'Français')}</option>
          <option value="es">{t('settings.language.es', 'Español')}</option>
          <option value="de">{t('settings.language.de', 'Deutsch')}</option>
          <option value="pt">{t('settings.language.pt', 'Português')}</option>
        </select>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.general.skillsDirectory', 'Skills Directory')}
        </label>
        <div className="flex flex-col gap-2">
          <div className="flex gap-2 max-w-lg">
            <Input
              value={effectiveDir}
              onChange={(e) => onSkillsDirectoryChange(e.target.value)}
              placeholder={t(
                'settings.general.skillsDirectoryPlaceholder',
                'Select a directory for local skills...',
              )}
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

          {/* Verification Status */}
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

      <SkillsListModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        skills={skills}
      />
    </div>
  );
}

export default React.memo(GeneralTabComponent, (prev, next) => {
  return (
    prev.localLanguage === next.localLanguage &&
    prev.skillsDirectory === next.skillsDirectory &&
    prev.onChange === next.onChange &&
    prev.onSkillsDirectoryChange === next.onSkillsDirectoryChange
  );
});
