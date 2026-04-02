import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { SkillMetadata } from '@/types/skills';
import useSWR from 'swr';
import { useEffect } from 'react';

const logger = getLogger('useSkillsDirectory');

export function useSkillsDirectory(
  skillsDirectory: string | undefined,
  onSkillsDirectoryChange: (path: string) => void,
) {
  // Fetch default directory if skillsDirectory is undefined or empty
  const { data: defaultDir } = useSWR<string>(
    !skillsDirectory ? 'get_default_skills_directory' : null,
    async () => await safeInvoke<string>('get_default_skills_directory'),
    {
      revalidateOnFocus: false,
      revalidateIfStale: false,
    }
  );

  // Sync default directory back to parent
  useEffect(() => {
    if (!skillsDirectory && defaultDir) {
      onSkillsDirectoryChange(defaultDir);
    }
  }, [skillsDirectory, defaultDir, onSkillsDirectoryChange]);

  const dirToVerify = skillsDirectory || '';

  const { data: skills, error, isLoading } = useSWR<SkillMetadata[]>(
    dirToVerify ? ['scan_skills_directory', dirToVerify] : null,
    async ([, dir]) => {
      return await safeInvoke<SkillMetadata[]>('scan_skills_directory', {
        directory: dir as string,
      });
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to verify skills directory', err);
      },
    }
  );

  let verificationStatus: 'idle' | 'loading' | 'success' | 'error' = 'idle';
  if (!dirToVerify) {
    verificationStatus = 'idle';
  } else if (isLoading) {
    verificationStatus = 'loading';
  } else if (error) {
    verificationStatus = 'error';
  } else if (skills) {
    verificationStatus = 'success';
  }

  const errorMessage = error instanceof Error ? error.message : String(error || '');

  return {
    verificationStatus,
    skills: skills || [],
    errorMessage,
  };
}
