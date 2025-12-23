import { useEffect } from 'react';
import { AgentClient } from '@/lib/agent-client';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { Message } from '@/models/chat';
import { MCPContent } from '@/lib/mcp-types';

export function useLLMResponder() {
  useEffect(() => {
    const unlistenPromise = AgentClient.listenToLlmRequest(async (payload) => {
      console.log('Received LLM Request from Agent:', payload);
      const { request_id, messages, system_prompt } = payload;

      try {
        // Dynamic import to avoid circular deps
        const { AIServiceFactory } = await import('@/lib/ai-service/factory');
        // const { LLMConfigManager } = await import('@/lib/llm-config-manager');

        // const configManager = new LLMConfigManager();
        // const config = configManager.getServiceConfig('openai');

        // Simplified config
        const config = {
          timeout: 30000,
        };

        // For now, hardcode OpenAI or prefer a specific provider if active
        // TODO: In Phase 4, pass provider/config from AgentConfig
        const service = AIServiceFactory.getService(
          AIServiceProvider.OpenAI,
          '',
          config,
        );

        const aiMessages: Message[] = messages.map((m) => {
          let content = m.content;
          if (Array.isArray(m.content)) {
            content = m.content.map((c) => {
              if (c.type === 'text') return { type: 'text', text: c.text };
              // Fallback for other types or strict casting if needed
              return { type: 'text', text: '' };
            });
          } else if (typeof content === 'string') {
            content = [{ type: 'text', text: content }];
          }

          // Constuct full Message object to satisfy interface
          return {
            id: crypto.randomUUID(),
            sessionId: 'thronglet-session', // Dummy
            threadId: 'thronglet-thread', // Dummy
            role: m.role as Message['role'],
            content: content as MCPContent[],
          };
        });

        // Execute Stream
        const generator = service.streamChat(aiMessages, {
          config: config,
          systemPrompt: system_prompt,
        });

        let accumulatedText = '';
        for await (const chunk of generator) {
          accumulatedText += chunk;
        }

        // Construct LLMResponse
        const response = {
          content: [{ type: 'text', text: accumulatedText }],
          tool_calls: null,
          usage: null,
        };

        // Submit back to Rust
        await AgentClient.submitLlmResponse(request_id, response);
      } catch (error) {
        console.error('LLM Responder failed:', error);
        await AgentClient.submitLlmResponse(request_id, {
          content: [{ type: 'text', text: `Error: ${error}` }],
          tool_calls: null,
          is_error: true,
        });
      }
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);
}
