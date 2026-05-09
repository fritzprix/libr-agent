import useSWR from 'swr';
import {
  listAssistantSummaries,
  type AssistantSummary,
} from '@/lib/backend/assistants';

export function useAssistantSummaries() {
  const {
    data: assistants = [],
    isLoading: loading,
    error,
  } = useSWR<AssistantSummary[]>('assistantSummaries', listAssistantSummaries);

  return { assistants, loading, error };
}
