import { getManagedSkillsOverview } from '@/lib/backend/skills';
import { getLogger } from '@/lib/logger';
import type { ManagedSkillsOverview } from '@/types/skills';
import useSWR from 'swr';

const logger = getLogger('useSkillsDirectory');

export function useSkillsDirectory() {
  const { data, error, isLoading, mutate } = useSWR<ManagedSkillsOverview>(
    'managed-skills-overview',
    async () => await getManagedSkillsOverview(),
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to load managed skills overview', err);
      },
    },
  );

  let verificationStatus: 'loading' | 'success' | 'error' = 'loading';
  if (error) {
    verificationStatus = 'error';
  } else if (!isLoading && data) {
    verificationStatus = 'success';
  }

  const errorMessage =
    error instanceof Error ? error.message : String(error || '');

  return {
    verificationStatus,
    overview: data,
    skills: data?.effectiveSkills ?? [],
    systemSkills: data?.systemSkills ?? [],
    userSkills: data?.userSkills ?? [],
    systemDirectory: data?.systemDirectory ?? '',
    userDirectory: data?.userDirectory ?? '',
    errorMessage,
    refresh: mutate,
  };
}
