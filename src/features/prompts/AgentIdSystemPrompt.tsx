import { useEffect, useRef } from 'react';
import { getLogger } from '@/lib/logger';
import { useSystemPrompt } from '@/context/SystemPromptContext';
import { useAssistantContext } from '@/context/AssistantContext';

const logger = getLogger('AgentIdSystemPrompt');

/**
 * Headless component that injects the current assistant's id (and name)
 * into the system prompt registry so MCP/LLM layers can access it.
 *
 * The component registers a system prompt under key `assistant-id` and
 * removes it when the assistant changes or the component unmounts.
 */
export function AgentIdSystemPrompt() {
  const { register, unregister } = useSystemPrompt();
  const { currentAssistant } = useAssistantContext();
  const registrationRef = useRef<string | null>(null);

  useEffect(() => {
    // Clean up previous registration if any
    if (registrationRef.current) {
      try {
        unregister(registrationRef.current);
      } catch (err) {
        logger.warn('Failed to unregister previous assistant-id prompt', err);
      }
      registrationRef.current = null;
    }

    if (!currentAssistant) {
      // nothing to register
      return;
    }

    const buildPrompt = () => {
      const id = currentAssistant.id ?? 'unknown';
      const name = currentAssistant.name ?? 'Unnamed Assistant';
      return `# Assistant Context\n- **Assistant ID**: ${id}\n- **Assistant Name**: ${name}\n\n*This identifier is provided for tooling/routing purposes.*`;
    };

    const id = register('assistant-id', buildPrompt, 2);
    registrationRef.current = id;
    logger.debug('Registered assistant-id system prompt', {
      promptId: id,
      assistantId: currentAssistant.id,
    });

    return () => {
      if (registrationRef.current) {
        try {
          unregister(registrationRef.current);
        } catch (err) {
          logger.warn(
            'Failed to unregister assistant-id prompt on cleanup',
            err,
          );
        }
        registrationRef.current = null;
      }
    };
  }, [currentAssistant, register, unregister]);

  return null;
}

export default AgentIdSystemPrompt;
