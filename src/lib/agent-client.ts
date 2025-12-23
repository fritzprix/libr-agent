import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface AgentConfig {
  name: string;
  systemPrompt: string;
  allowedMcpServers: string[];
  initialHistory?: unknown[];
}

export interface AgentLlmRequestPayload {
  request_id: string;
  messages: {
    role: string;
    content:
      | string
      | Array<{ type: string; text?: string; [key: string]: unknown }>;
  }[];
  system_prompt?: string;
}

export class AgentClient {
  static async start(config: AgentConfig): Promise<string> {
    const rustConfig = {
      name: config.name,
      system_prompt: config.systemPrompt,
      allowed_mcp_servers: config.allowedMcpServers,
      initial_history: config.initialHistory,
    };

    return await invoke('agent_start', { config: rustConfig });
  }

  static async submitLlmResponse(
    requestId: string,
    response: unknown,
  ): Promise<void> {
    await invoke('agent_llm_response', { requestId, response });
  }

  static async listenToLlmRequest(
    callback: (payload: AgentLlmRequestPayload) => void,
  ): Promise<UnlistenFn> {
    return await listen<AgentLlmRequestPayload>(
      'agent://llm_request',
      (event) => {
        callback(event.payload);
      },
    );
  }
}
