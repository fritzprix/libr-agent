import { safeInvoke } from '@/lib/backend/core';
import { useEffect } from 'react';
import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import type { SkillMetadata } from '@/types/skills';

const logger = getLogger('useSkillsDirectory');

export function useSkillsDirectory(
  skillsDirectory: string | undefined,
  onSkillsDirectoryChange: (path: string) => void,
) {
  // Use a separate SWR for fetching the default directory to handle the "no directory set" case cleanly
  const { data: defaultDirectory } = useSWR(
    !skillsDirectory ? 'default-skills-directory' : null,
    async () => {
      try {
        return await safeInvoke<string>('get_default_skills_directory');
      } catch (error) {
        logger.warn('Failed to get default skills directory', error);
        return null;
      }
    },
    { revalidateOnFocus: false },
  );

  useEffect(() => {
    if (!skillsDirectory && defaultDirectory) {
      onSkillsDirectoryChange(defaultDirectory);
    }
  }, [skillsDirectory, defaultDirectory, onSkillsDirectoryChange]);

  // Main SWR for verifying and scanning the skills directory
  const { data, error, isLoading } = useSWR(
    skillsDirectory ? ['scan-skills-directory', skillsDirectory] : null,
    async ([, dir]) => {
      return await safeInvoke<SkillMetadata[]>('scan_skills_directory', {
        directory: dir,
      });
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to verify skills directory', err);
      }
    },
  );

  let verificationStatus: 'idle' | 'loading' | 'success' | 'error' = 'idle';
  if (isLoading) {
    verificationStatus = 'loading';
  } else if (error) {
    verificationStatus = 'error';
  } else if (data) {
    verificationStatus = 'success';
  } else if (!skillsDirectory) {
    verificationStatus = 'idle';
  }

  const errorMessage = error
    ? error instanceof Error
      ? error.message
      : String(error)
    : '';

  return {
    verificationStatus,
    skills: data || [],
    errorMessage,
  };
}
