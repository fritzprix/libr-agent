import { useEffect, useState } from 'react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
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
import { SkillMetadata } from '@/types/skills';

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
  const [verificationStatus, setVerificationStatus] = useState<
    'idle' | 'loading' | 'success' | 'error'
  >('idle');
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isModalOpen, setIsModalOpen] = useState(false);

  useEffect(() => {
    async function verifySkills() {
      if (!skillsDirectory) {
        try {
          // If no directory is set, try to get the default one
          const defaultDir = await invoke<string>(
            'get_default_skills_directory',
          );
          onSkillsDirectoryChange(defaultDir);
          return; // The change will trigger the effect again
        } catch (error) {
          logger.warn('Failed to get default skills directory', error);
          // Fall through to empty state
        }
      }

      const dirToVerify = skillsDirectory || '';

      if (!dirToVerify) {
        setVerificationStatus('idle');
        setSkills([]);
        return;
      }

      setVerificationStatus('loading');
      try {
        const result = await invoke<SkillMetadata[]>('scan_skills_directory', {
          directory: dirToVerify,
        });
        setSkills(result);
        setVerificationStatus('success');
      } catch (error) {
        logger.error('Failed to verify skills directory', error);
        setVerificationStatus('error');
        setErrorMessage(error instanceof Error ? error.message : String(error));
        setSkills([]);
      }
    }

    verifySkills();
  }, [skillsDirectory]);

  const handleBrowseEvents = async () => {
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
    }
  };

  const handleOpenDirectory = async () => {
    if (!skillsDirectory) {
      logger.warn('handleOpenDirectory called but skillsDirectory is empty');
      return;
    }
    logger.info(`Attempting to open directory: ${skillsDirectory}`);
    try {
      await invoke('open_skills_directory_in_explorer', {
        directory: skillsDirectory,
      });
      logger.info(`Successfully requested open_skills_directory_in_explorer`);
    } catch (error) {
      logger.error('Failed to open directory', error);
      // Optional: show toast error
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
          <option value="en">
            {t('settings.language.english', 'English')}
          </option>
          <option value="ko">{t('settings.language.korean', 'Korean')}</option>
        </select>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.general.skillsDirectory', 'Skills Directory')}
        </label>
        <div className="flex flex-col gap-2">
          <div className="flex gap-2 max-w-lg">
            <Input
              value={skillsDirectory || ''}
              onChange={(e) => onSkillsDirectoryChange(e.target.value)}
              placeholder={t(
                'settings.general.skillsDirectoryPlaceholder',
                'Select a directory for local skills...',
              )}
              className="bg-background border text-foreground flex-1"
            />
            <Button
              variant="outline"
              onClick={handleBrowseEvents}
              title={t('settings.general.browse', 'Browse')}
              className="px-3"
              aria-label={t(
                'settings.general.browseAriaLabel',
                'Browse skills directory',
              )}
            >
              <FolderOpen className="w-4 h-4" />
            </Button>
            {skillsDirectory && (
              <Button
                variant="outline"
                onClick={handleOpenDirectory}
                title={t('settings.general.openInExplorer', 'Open in Explorer')}
                className="px-3"
              >
                <FolderOutput className="w-4 h-4" />
              </Button>
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
                className="flex items-center gap-2 hover:underline focus:outline-none"
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
