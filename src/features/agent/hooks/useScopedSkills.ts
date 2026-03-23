import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { getAggregatedSkills } from '@/lib/backend/skills';
import type { SkillMetadata } from '@/types/skills';

const logger = getLogger('useScopedSkills');

interface ScopedSkillsOptions {
  assistantId?: string;
  workspacePath?: string | null;
  /** When provided, the backend resolves workspace scope from the session. */
  sessionId?: string | null;
}

export function useScopedSkills(
  assistantIdOrOptions?: string | ScopedSkillsOptions,
  workspacePath?: string | null,
): {
  skills: SkillMetadata[];
  isLoading: boolean;
} {
  // Normalise the overloaded first argument
  const assistantId =
    typeof assistantIdOrOptions === 'string'
      ? assistantIdOrOptions
      : assistantIdOrOptions?.assistantId;
  const resolvedWorkspacePath =
    typeof assistantIdOrOptions === 'string'
      ? workspacePath
      : assistantIdOrOptions?.workspacePath;
  const sessionId =
    typeof assistantIdOrOptions === 'object' && assistantIdOrOptions !== null
      ? assistantIdOrOptions.sessionId
      : undefined;

  const { data, isLoading } = useSWR(
    [
      'scoped-skills',
      assistantId ?? '',
      resolvedWorkspacePath ?? '',
      sessionId ?? '',
    ],
    async ([, scopedAssistantId, scopedWorkspacePath, scopedSessionId]) => {
      try {
        return await getAggregatedSkills(scopedAssistantId || undefined, {
          workspacePath: scopedWorkspacePath || undefined,
          sessionId: scopedSessionId || undefined,
        });
      } catch (error) {
        logger.error('Failed to load scoped skills', error);
        return [];
      }
    },
    {
      revalidateOnFocus: true,
      shouldRetryOnError: false,
    },
  );

  return {
    skills: data ?? [],
    isLoading,
  };
}
