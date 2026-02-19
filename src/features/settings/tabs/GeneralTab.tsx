import { useEffect, useState } from 'react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui';
import { FolderOutput, Loader2, CheckCircle, AlertCircle } from 'lucide-react';
import { SkillsListModal } from '../components/SkillsListModal';
import { getLogger } from '@/lib/logger';

const logger = getLogger('GeneralTab');

interface SkillMetadata {
  name: string;
  description: string;
  path: string;
}

interface GeneralTabProps {
  localLanguage: string;
  onChange: (lang: string) => void;
}

function GeneralTabComponent({ localLanguage, onChange }: GeneralTabProps) {
  const { t } = useTranslation('common');
  const [verificationStatus, setVerificationStatus] = useState<
    'idle' | 'loading' | 'success' | 'error'
  >('idle');
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [skillsDirectory, setSkillsDirectory] = useState<string>('');

  useEffect(() => {
    async function loadSkills() {
      setVerificationStatus('loading');
      try {
        // Get default skills directory (auto-copied from bundled_skills)
        const defaultDir = await invoke<string>('get_default_skills_directory');
        setSkillsDirectory(defaultDir);

        // Scan skills in the directory
        const result = await invoke<SkillMetadata[]>('scan_skills_directory', {
          directory: defaultDir,
        });
        setSkills(result);
        setVerificationStatus('success');
      } catch (error) {
        logger.error('Failed to load skills', error);
        setVerificationStatus('error');
        setErrorMessage(String(error));
        setSkills([]);
      }
    }

    loadSkills();
  }, []);

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
          {t('settings.general.globalSkills', 'Global Skills')}
        </label>
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            {verificationStatus === 'loading' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                <span className="text-sm text-muted-foreground">
                  {t('settings.general.loadingSkills', 'Loading skills...')}
                </span>
              </>
            )}
            {verificationStatus === 'success' && (
              <>
                <CheckCircle className="w-4 h-4 text-success" />
                <span className="text-sm text-muted-foreground">
                  {skills.length}{' '}
                  {t('settings.general.skillsFound', 'skills available')}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setIsModalOpen(true)}
                  className="ml-2"
                >
                  {t('settings.general.viewSkills', 'View Skills')}
                </Button>
                {skillsDirectory && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleOpenDirectory}
                    title={t(
                      'settings.general.openInExplorer',
                      'Open in Explorer',
                    )}
                  >
                    <FolderOutput className="w-4 h-4" />
                  </Button>
                )}
              </>
            )}
            {verificationStatus === 'error' && (
              <>
                <AlertCircle className="w-4 h-4 text-destructive" />
                <span className="text-sm text-destructive">
                  {errorMessage ||
                    t(
                      'settings.general.skillsLoadError',
                      'Failed to load skills',
                    )}
                </span>
              </>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.general.globalSkillsDescription',
              'Global skills are automatically provided with LibrAgent. Assistant-specific skills can be configured per assistant.',
            )}
          </p>
          {skillsDirectory && (
            <p className="text-xs text-muted-foreground/70">
              {t('settings.general.skillsLocation', 'Location')}:{' '}
              {skillsDirectory}
            </p>
          )}
        </div>
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
    prev.localLanguage === next.localLanguage && prev.onChange === next.onChange
  );
});
