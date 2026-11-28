import { safeInvoke } from './core';

/**
 * One model as `llm-client` reports it (#3091).
 *
 * Fields a provider's list API does not report are absent; the caller fills
 * them in from the static LLM config, as the provider services used to.
 */
export interface ListedModel {
  id: string;
  name: string;
  description: string;
  contextWindow?: number;
  supportReasoning?: boolean;
  supportTools?: boolean;
  supportStreaming?: boolean;
  cost?: { input: number; output: number };
}

/**
 * Calls the Rust `list_llm_models` command: the provider's own model list
 * API, from the native client (same proxy rules as chat).
 */
export async function listLlmModels(args: {
  providerId: string;
  timeoutMs: number;
}): Promise<ListedModel[]> {
  return safeInvoke<ListedModel[]>('list_llm_models', { ...args });
}
