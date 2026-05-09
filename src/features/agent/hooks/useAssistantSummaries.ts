import useSWR from 'swr';
import {
  listAssistantSummaries,
  type AssistantSummary,
} from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAssistantSummaries');

export function useAssistantSummaries() {
  const {
    data: assistants = [],
    isLoading: loading,
    error,
  } = useSWR<AssistantSummary[]>('assistantSummaries', listAssistantSummaries, {
    revalidateOnFocus: false,
    revalidateOnReconnect: false,
    shouldRetryOnError: false,
    onError: (err) => {
      logger.warn('Failed to load assistant summaries', err);
    },
  });

  return { assistants, loading, error };
}
