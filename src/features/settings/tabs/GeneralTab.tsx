import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Button, Input } from '@/components/ui';
import { FolderOpen, Loader2, CheckCircle, AlertCircle } from 'lucide-react';
import { useDebounced } from '@/hooks/useDebounced';
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
  skillsDirectory?: string;
  onSkillsDirectoryChange: (path: string) => void;
}

export function GeneralTab({
  localLanguage,
  onChange,
  skillsDirectory,
  onSkillsDirectoryChange,
}: GeneralTabProps) {
  const { t } = useTranslation('common');
  const [verificationStatus, setVerificationStatus] = useState<
    'idle' | 'loading' | 'success' | 'error' | 'missing'
  >('idle');
  const [skillCount, setSkillCount] = useState<number>(0);
  const [errorMessage, setErrorMessage] = useState<string>('');

  const debouncedSkillsDirectory = useDebounced(skillsDirectory, 500);

  useEffect(() => {
    async function verifySkills() {
      if (!debouncedSkillsDirectory) {
        setVerificationStatus('idle');
        return;
      }

      setVerificationStatus('loading');
      try {
        const skills = await invoke<SkillMetadata[]>('scan_skills_directory', {
          directory: debouncedSkillsDirectory,
        });
        setSkillCount(skills.length);
        setVerificationStatus('success');
      } catch (error) {
        const errorStr = String(error);
        logger.error('Failed to verify skills directory', error);
        
        // Check if error is about directory not existing
        if (errorStr.includes('does not exist')) {
          setVerificationStatus('missing');
          setErrorMessage('Directory does not exist');
        } else {
          setVerificationStatus('error');
          setErrorMessage(errorStr);
        }
      }
    }

    verifySkills();
  }, [debouncedSkillsDirectory]);

  const handleCreateDirectory = async () => {
    if (!skillsDirectory) return;
    
    try {
      setVerificationStatus('loading');
      await invoke('create_skills_directory', {
        directory: skillsDirectory,
      });
      
      // Re-verify after creation
      const skills = await invoke<SkillMetadata[]>('scan_skills_directory', {
        directory: skillsDirectory,
      });
      setSkillCount(skills.length);
      setVerificationStatus('success');
    } catch (error) {
      logger.error('Failed to create skills directory', error);
      setVerificationStatus('error');
      setErrorMessage(String(error));
    }
  };

  const handleBrowseEvents = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Skills Directory',
      });

      if (selected && typeof selected === 'string') {
        onSkillsDirectoryChange(selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
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
              placeholder="Select a directory for local skills..."
              className="bg-background border text-foreground flex-1"
            />
            <Button
              variant="outline"
              onClick={handleBrowseEvents}
              title="Browse"
              aria-label="Browse for skills directory"
              className="px-3"
            >
              <FolderOpen className="w-4 h-4" />
            </Button>
          </div>

          {/* Verification Status */}
          <div className="flex items-center gap-2 text-sm h-5">
            {verificationStatus === 'loading' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                <span className="text-muted-foreground">Verifying...</span>
              </>
            )}
            {verificationStatus === 'success' && (
              <>
                <CheckCircle className="w-4 h-4 text-green-500" />
                <span className="text-green-500">
                  Found {skillCount} skill{skillCount !== 1 ? 's' : ''}
                </span>
              </>
            )}
            {verificationStatus === 'missing' && (
              <>
                <AlertCircle className="w-4 h-4 text-yellow-500" />
                <span className="text-yellow-500">Directory does not exist</span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleCreateDirectory}
                  className="ml-2 h-6 px-2 text-xs"
                >
                  Create Directory
                </Button>
              </>
            )}
            {verificationStatus === 'error' && (
              <>
                <AlertCircle className="w-4 h-4 text-red-500" />
                <span className="text-red-500">
                  {errorMessage || 'Invalid directory'}
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
    </div>
  );
}
