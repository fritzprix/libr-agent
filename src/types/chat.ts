import { MCPTool } from "../lib/tauri-mcp-client";

export interface Message {
  id: string;
  sessionId: string; // Added sessionId
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  isStreaming?: boolean;
  thinking?: string;
  assistantId?: string; // Optional, used for tracking in multi-agent scenarios
  attachments?: { name: string; content: string }[];
  tool_use?: { id: string; name: string; input: Record<string, unknown> };
  function_call?: { name: string; arguments: Record<string, unknown> };
  createdAt?: Date; // Added
  updatedAt?: Date; // Added
}

export interface ToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: string;
  };
}

export interface StreamableMessage extends Message {}

export interface Assistant {
  id: string;
  name: string;
  systemPrompt: string;
  mcpConfig: {
    mcpServers?: Record<
      string,
      {
        command: string;
        args?: string[];
        env?: Record<string, string>;
      }
    >;
  };
  localServices?: string[];
  isDefault: boolean;
  createdAt: Date;
  updatedAt: Date;
}

export interface Tool extends MCPTool {
  isLocal?: boolean;
}

export interface Session {
  id: string;
  type: "single" | "group";
  assistants: Assistant[];
  name?: string; // Group 세션의 경우 그룹명
  description?: string; // Group 세션의 경우 설명
  createdAt: Date;
  updatedAt: Date;
}

export interface Group {
  id: string;
  name: string;
  description: string;
  assistants: Assistant[];
  createdAt: Date;
}
