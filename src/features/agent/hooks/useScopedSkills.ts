import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { getAggregatedSkills } from '@/lib/backend/skills';
import type { SkillMetadata } from '@/types/skills';

const logger = getLogger('useScopedSkills');

export function useScopedSkills(
  assistantId?: string,
  workspacePath?: string | null,
): {
  skills: SkillMetadata[];
  isLoading: boolean;
} {
  const { data, isLoading } = useSWR(
    ['scoped-skills', assistantId ?? '', workspacePath ?? ''],
    async ([, scopedAssistantId, scopedWorkspacePath]) => {
      try {
        return await getAggregatedSkills(scopedAssistantId || undefined, {
          workspacePath: scopedWorkspacePath || undefined,
        });
      } catch (error) {
        logger.error('Failed to load scoped skills', error);
        return [];
      }
    },
    {
      revalidateOnFocus: false,
      shouldRetryOnError: false,
    },
  );

  return {
    skills: data ?? [],
    isLoading,
  };
}
