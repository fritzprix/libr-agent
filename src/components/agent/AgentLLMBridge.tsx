import { useLLMResponder } from '@/hooks/use-llm-responder';

export function AgentLLMBridge() {
  useLLMResponder();
  return null; // Headless component
}
