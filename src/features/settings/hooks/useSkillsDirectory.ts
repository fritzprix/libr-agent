import { safeInvoke } from '@/lib/backend/core';
import { useEffect, useState } from 'react';
import { getLogger } from '@/lib/logger';
import type { SkillMetadata } from '@/types/skills';

const logger = getLogger('useSkillsDirectory');

export function useSkillsDirectory(
  skillsDirectory: string | undefined,
  onSkillsDirectoryChange: (path: string) => void,
) {
  const [verificationStatus, setVerificationStatus] = useState<
    'idle' | 'loading' | 'success' | 'error'
  >('idle');
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [errorMessage, setErrorMessage] = useState<string>('');

  useEffect(() => {
    let isActive = true;

    async function verifySkills() {
      if (!skillsDirectory) {
        try {
          // If no directory is set, try to get the default one
          const defaultDir = await safeInvoke<string>(
            'get_default_skills_directory',
          );
          if (!isActive) return;
          onSkillsDirectoryChange(defaultDir);
          return; // The change will trigger the effect again
        } catch (error) {
          logger.warn('Failed to get default skills directory', error);
          // Fall through to empty state
        }
      }

      const dirToVerify = skillsDirectory || '';

      if (!dirToVerify) {
        if (!isActive) return;
        setVerificationStatus('idle');
        setErrorMessage('');
        setSkills([]);
        return;
      }

      if (!isActive) return;
      setVerificationStatus('loading');
      setErrorMessage('');
      try {
        const result = await safeInvoke<SkillMetadata[]>(
          'scan_skills_directory',
          {
            directory: dirToVerify,
          },
        );
        if (!isActive) return;
        setSkills(result);
        setVerificationStatus('success');
        setErrorMessage('');
      } catch (error) {
        if (!isActive) return;
        logger.error('Failed to verify skills directory', error);
        setVerificationStatus('error');
        setErrorMessage(error instanceof Error ? error.message : String(error));
        setSkills([]);
      }
    }

    void verifySkills();

    return () => {
      isActive = false;
    };
  }, [skillsDirectory, onSkillsDirectoryChange]);

  return {
    verificationStatus,
    skills,
    errorMessage,
  };
}
