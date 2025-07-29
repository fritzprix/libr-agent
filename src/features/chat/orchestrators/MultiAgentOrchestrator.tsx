import { useAssistantContext } from '@/context/AssistantContext';
import { LocalService, MCPResponse, useLocalTools } from '@/context/LocalToolContext';
import { useSessionContext } from '@/context/SessionContext';
import { useSessionHistory } from '@/context/SessionHistoryContext';
import { useChatContext } from '@/hooks/use-chat';
import { Assistant } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useCallback, useEffect, useMemo, useRef } from 'react';

const MULTI_AGENT_ORCHESTRATOR_ASSISTANT_ID = 'multi-agent-orchestrator';
const MULTI_AGENT_SERVICE = 'multi-agent-orchestrator-service';

interface PromptToUserInput {
  prompt: string;
}

interface SwitchAssistantInput {
  assistantId: string;
  instruction: string;
}

interface SetPlanInput {
  items: string[];
}

interface CheckPlanItemInput {
  index: number;
}

interface ReportResultInput {
  resultInDetail: string;
}

interface PlanItem {
  plan: string;
  complete: boolean;
}

export const MultiAgentOrchestrator: React.FC = () => {
  const { currentAssistant, setCurrentAssistant, assistants } = useAssistantContext();
  const { registerService, unregisterService } = useLocalTools();
  const { current: currentSession } = useSessionContext();
  const { addMessage } = useSessionHistory();
  const { submit } = useChatContext();

  const plan = useRef<PlanItem[]>([]);

  // ✨ Clean, stable tool handlers using the simpler useStableHandler
  const handlePromptToUser = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { prompt } = args as PromptToUserInput;
      const id = createId();
      if (!currentSession) {
        return {
          jsonrpc: '2.0',
          id,
          error: {
            code: 400,
            message: 'No active session',
          },
        };
      }
      addMessage({
        assistantId: MULTI_AGENT_ORCHESTRATOR_ASSISTANT_ID,
        id,
        content: prompt,
        role: 'assistant',
        sessionId: currentSession.id,
      });
      return {
        jsonrpc: '2.0',
        id,
        result: { message: 'Prompt sent to user' },
      };
    },
    [addMessage, currentSession],
  );

  const handleSwitchAssistant = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { assistantId, instruction } = args as SwitchAssistantInput;
      const id = createId();
      if (!currentSession) {
        return {
          jsonrpc: '2.0',
          id,
          error: {
            code: 400,
            message: 'No active session',
          },
        };
      }
      const nextAssistant = assistants.find((a) => a.id === assistantId);
      if (nextAssistant) {
        setCurrentAssistant(nextAssistant);
        submit([
          {
            id,
            assistantId: MULTI_AGENT_ORCHESTRATOR_ASSISTANT_ID,
            content: instruction,
            role: 'user',
            sessionId: currentSession.id,
          },
        ]);
        return {
          jsonrpc: '2.0',
          id,
          result: { success: true, message: `Switched to assistant: ${nextAssistant.name}` },
        };
      } else {
        return {
          jsonrpc: '2.0',
          id,
          error: {
            code: 404,
            message: `Assistant with ID ${assistantId} not found`,
          },
        };
      }
    },
    [submit, currentSession, assistants, setCurrentAssistant],
  );

  const handleSetPlan = useCallback(async (args: unknown): Promise<MCPResponse> => {
    const { items } = args as SetPlanInput;
    const id = createId();
    const newPlan = items.map(
      (item) => ({ plan: item, complete: false }) satisfies PlanItem,
    );
    plan.current = newPlan;
    return {
      jsonrpc: '2.0',
      id,
      result: { message: `Plan set with ${items.length} items`, items },
    };
  }, []);

  const handleCheckPlanItem = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { index } = args as CheckPlanItemInput;
      const id = createId();
      if (index >= 0 && index < plan.current.length) {
        plan.current[index].complete = true;
        return {
          jsonrpc: '2.0',
          id,
          result: { message: `Plan item ${index} marked as complete`, index },
        };
      } else {
        return {
          jsonrpc: '2.0',
          id,
          error: {
            code: 400,
            message: `Invalid plan item index: ${index}`,
          },
        };
      }
    },
    [],
  );

  const handleClearPlan = useCallback(async (): Promise<MCPResponse> => {
    const id = createId();
    plan.current = [];
    return {
      jsonrpc: '2.0',
      id,
      result: { message: 'Plan cleared' },
    };
  }, []);

  const handleReportResult = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { resultInDetail } = args as ReportResultInput;
      const id = createId();
      if (!currentSession) {
        return {
          jsonrpc: '2.0',
          id,
          error: {
            code: 400,
            message: 'No active session',
          },
        };
      }
      addMessage({
        id,
        assistantId: MULTI_AGENT_ORCHESTRATOR_ASSISTANT_ID,
        content: resultInDetail,
        role: 'assistant',
        sessionId: currentSession.id,
      });
      return {
        jsonrpc: '2.0',
        id,
        result: { message: 'Result reported', detail: resultInDetail },
      };
    },
    [addMessage, currentSession],
  );

  // ✨ Now localService is stable - handlers never change
  const localService: LocalService = useMemo(
    () => ({
      name: MULTI_AGENT_SERVICE,
      tools: [
        {
          toolDefinition: {
            name: 'promptToUser',
            description:
              'Prompt the user for additional information or clarification',
            input_schema: {
              type: 'object',
              properties: {
                prompt: {
                  type: 'string',
                  description: 'The prompt message to show to the user',
                },
              },
              required: ['prompt'],
            },
          },
          handler: handlePromptToUser,
        },
        {
          toolDefinition: {
            name: 'switchAssistant',
            description:
              'Switch to a different specialized assistant with specific instructions',
            input_schema: {
              type: 'object',
              properties: {
                assistantId: {
                  type: 'string',
                  description: 'The ID of the assistant to switch to',
                },
                instruction: {
                  type: 'string',
                  description: 'Clear instructions for the new assistant',
                },
              },
              required: ['assistantId', 'instruction'],
            },
          },
          handler: handleSwitchAssistant,
        },
        {
          toolDefinition: {
            name: 'setPlan',
            description: 'Set a plan of action items for the user',
            input_schema: {
              type: 'object',
              properties: {
                items: {
                  type: 'array',
                  items: { type: 'string' },
                  description: 'Array of plan items/steps',
                },
              },
              required: ['items'],
            },
          },
          handler: handleSetPlan,
        },
        {
          toolDefinition: {
            name: 'checkPlanItem',
            description: 'Mark a specific plan item as completed',
            input_schema: {
              type: 'object',
              properties: {
                index: {
                  type: 'number',
                  description:
                    '0-based index of the plan item to mark as complete',
                },
              },
              required: ['index'],
            },
          },
          handler: handleCheckPlanItem,
        },
        {
          toolDefinition: {
            name: 'clearPlan',
            description: 'Clear/cancel the current plan',
            input_schema: {
              type: 'object',
              properties: {},
              required: [],
            },
          },
          handler: handleClearPlan,
        },
        {
          toolDefinition: {
            name: 'reportResult',
            description:
              'Provide a detailed summary of completed task or current status',
            input_schema: {
              type: 'object',
              properties: {
                resultInDetail: {
                  type: 'string',
                  description: 'Detailed result summary',
                },
              },
              required: ['resultInDetail'],
            },
          },
          handler: handleReportResult,
        },
      ],
    }),
    [
      handlePromptToUser,
      handleSwitchAssistant,
      handleSetPlan,
      handleCheckPlanItem,
      handleClearPlan,
      handleReportResult,
    ],
  );

  const multiAgentOrchestratorAssistant: Assistant = useMemo(
    () => ({
      id: MULTI_AGENT_ORCHESTRATOR_ASSISTANT_ID,
      name: 'MultiAgentOrchestrator',
      systemPrompt: `You are a Multi-Agent Orchestrator. Your goal is to coordinate multiple specialized AI assistants to fulfill complex user requests. You have access to the following tools to manage the workflow:

1. **promptToUser(prompt: string)**: Use this to ask the user for additional information or clarification. This will pause the current operation and wait for user input.
2. **switchAssistant(assistantId: string, instruction: string)**: Use this to delegate a task to another specialized assistant. Provide the assistant ID and clear instructions for the new assistant. The current conversation context will be passed to the new assistant.
3. **setPlan(items: string[])**: Use this to outline a plan of action for the user in a checklist format. Each item in the array should be a step in the plan.
4. **checkPlanItem(index: number)**: Use this to mark a specific item in the current plan as completed. The index is 0-based.
5. **clearPlan()**: Use this to cancel and remove the current plan.
6. **reportResult(resultInDetail: string)**: Use this to provide a detailed summary of the completed task or the current status to the user. This will conclude the current orchestration cycle.

Your primary objective is to break down complex requests, delegate to appropriate assistants, manage the overall plan, and report back to the user clearly and concisely. Always consider the most efficient way to achieve the user's goal.

Available assistants: ${assistants.map((a) => `${a.id}: ${a.name}`).join(', ')}`,
      localServices: [localService.name],
      mcpConfig: {},
      isDefault: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    }),
    [localService, assistants],
  );

  // ✨ Now this useEffect won't cause infinite loops
  useEffect(() => {
    registerService(localService);
    return () => {
      unregisterService(MULTI_AGENT_SERVICE);
    };
  }, [localService]); // localService is now stable

  useEffect(() => {
    setCurrentAssistant(multiAgentOrchestratorAssistant);
  }, [multiAgentOrchestratorAssistant, setCurrentAssistant]);

  return (
    <div className="multi-agent-orchestrator-container p-4 border-t border-gray-700 bg-gray-800 text-foreground rounded-lg shadow-lg mb-4">
      <h3 className="text-lg font-semibold flex items-center">
        Multi-Agent Orchestrator Active
      </h3>
      <div className="mt-4">
        <div className="text-sm text-gray-300 mb-3">
          The Multi-Agent Orchestrator is coordinating specialized assistants to
          handle complex tasks. Current assistant:{' '}
          <span className="font-semibold text-blue-300">
            {currentAssistant?.name}
          </span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
          <div className="p-3 bg-gray-700 rounded-lg">
            <h4 className="text-sm font-semibold mb-2">Available Assistants</h4>
            <div className="text-xs text-gray-300 space-y-1">
              {assistants.map((assistant) => (
                <div
                  key={assistant.id}
                  className={`p-1 rounded ${
                    assistant.id === currentAssistant?.id ? 'bg-blue-600' : ''
                  }`}
                >
                  {assistant.name} ({assistant.id})
                </div>
              ))}
            </div>
          </div>

          <div className="p-3 bg-gray-700 rounded-lg">
            <h4 className="text-sm font-semibold mb-2">Orchestrator Tools</h4>
            <div className="text-xs text-gray-300 space-y-1">
              <div>• Prompt user for input</div>
              <div>• Switch between assistants</div>
              <div>• Manage task plans</div>
              <div>• Report results</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
