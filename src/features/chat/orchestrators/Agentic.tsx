import { useAssistantContext } from '@/context/AssistantContext';
import {
  LocalService,
  MCPResponse,
  useLocalTools,
} from '@/context/LocalToolContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/hooks/use-chat';
import { getLogger } from '@/lib/logger';
import { createObjectSchema, createStringSchema } from '@/lib/tauri-mcp-client';
import { Assistant, Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';

const logger = getLogger('Agentic.tsx');

interface SwitchAssistantInputType {
  from: string;
  to: string;
  query: string;
}

interface ReportToUserInputType {
  report: string;
  isComplete?: string; // "true" | "false" | undefined
}

const SUPERVISER_ASSISTANT_ID = 'superviser-assistant';

export function SimpleAgenticFlow() {
  const { current: currentSession } = useSessionContext();
  const [error, setError] = useState<string | null>(null);
  const flowStepCountRef = useRef(0);
  const maxStepsRef = useRef(10);
  const lastReportRef = useRef<boolean>(false);

  const { submit, messages } = useChatContext();
  const {
    assistants,
    setCurrentAssistant,
    getCurrentAssistant,
    registerEphemeralAssistant,
    unregisterEphemeralAssistant,
  } = useAssistantContext();
  const { idle } = useScheduler();
  const { unregisterService, registerService } = useLocalTools();

  // 🎯 Worker의 공개 정보만 요약하기
  const getWorkerInfoSummary = useCallback(() => {
    if (!currentSession?.assistants) {
      return 'No worker assistant available.';
    }

    const worker = currentSession.assistants.find(
      (assistant) => assistant.id !== SUPERVISER_ASSISTANT_ID,
    );

    if (!worker) {
      return 'No worker assistant available.';
    }

    // 🎯 적절한 정보만 추출
    const info = [];
    info.push(`Name: "${worker.name}"`);

    if (worker.description) {
      info.push(`Description: ${worker.description}`);
    }

    if (worker.localServices && worker.localServices.length > 0) {
      info.push(`Local Services: ${worker.localServices.join(', ')}`);
    }

    if (worker.mcpConfig.mcpServers) {
      const mcpCapabilities = Object.keys(worker.mcpConfig.mcpServers);
      if (mcpCapabilities.length > 0) {
        info.push(`MCP Capabilities: ${mcpCapabilities.join(', ')}`);
      }
    }

    return info.join('\n');
  }, [currentSession]);

  // 🎯 단일 Worker 인스턴스 가져오기
  const getWorkerInstance = useCallback(() => {
    if (!currentSession?.assistants) {
      return null;
    }

    const worker = currentSession.assistants.find(
      (assistant) => assistant.id !== SUPERVISER_ASSISTANT_ID,
    );

    return worker || null;
  }, [currentSession]);

  const handleSwitchAssistant = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      try {
        const { from, to, query } = args as SwitchAssistantInputType;

        if (!from || !to || !query) {
          throw new Error('Missing required fields: from, to, query');
        }

        flowStepCountRef.current += 1;
        logger.info(`[Agentic Flow] Switching assistant: ${from} → ${to}`);
        logger.info(`- Query: ${query}`);

        // 🎯 대상 Assistant 찾기
        let targetAssistant: Assistant | null = null;

        if (to === SUPERVISER_ASSISTANT_ID) {
          targetAssistant = superviserAssistant;
        } else {
          targetAssistant = getWorkerInstance();
        }

        if (targetAssistant) {
          logger.info(
            `[Agentic Flow] Switching to assistant: ${targetAssistant.name}`,
          );

          // Assistant 전환 (불필요한 delay 제거)
          setCurrentAssistant(targetAssistant);
        } else {
          logger.error(`[Agentic Flow] Target assistant not found: ${to}`);
        }

        // 최대 단계 수 체크
        if (flowStepCountRef.current >= maxStepsRef.current) {
          logger.warn(
            '[Agentic Flow] Maximum switches reached, suggesting completion',
          );
          return {
            id: createId(),
            jsonrpc: '2.0',
            success: true,
            result: {
              content: [
                {
                  type: 'text',
                  text: `Maximum assistant switches reached. Please consider using report_to_user to finish the task.`,
                },
              ],
            },
          };
        }

        return {
          id: createId(),
          jsonrpc: '2.0',
          success: true,
          result: {
            content: [
              {
                type: 'text',
                text: `Switching to ${to}: ${query}`,
              },
            ],
          },
        };
      } catch (error) {
        logger.error('[Agentic Flow] Switch assistant error:', error);
        return {
          id: createId(),
          jsonrpc: '2.0',
          success: false,
          error: {
            code: -1,
            message: error instanceof Error ? error.message : 'Unknown error',
          },
        };
      }
    },
    [getWorkerInstance, setCurrentAssistant],
  );

  const handleReportToUser = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      try {
        const rawArgs = args as ReportToUserInputType;
        const { report } = rawArgs;
        // 더 robust한 isComplete 처리
        const isComplete =
          rawArgs.isComplete === 'true' || rawArgs.isComplete === undefined;

        if (!report) {
          throw new Error('Report content is required');
        }

        logger.info('[Agentic Flow] Reporting to user:', report);
        logger.info(
          `[Agentic Flow] Task completion status: ${isComplete ? 'Complete' : 'In Progress'}`,
        );
        logger.info(
          `[Agentic Flow] Flow completed in ${flowStepCountRef.current} steps`,
        );

        // 리포트 플래그 설정
        lastReportRef.current = true;
        flowStepCountRef.current = 0;

        return {
          id: createId(),
          jsonrpc: '2.0',
          success: true,
          result: {
            content: [
              {
                type: 'text',
                text: `Final Report: ${report}`,
              },
            ],
          },
        };
      } catch (error) {
        logger.error('[Agentic Flow] Report error:', error);
        setError(error instanceof Error ? error.message : 'Unknown error');
        return {
          id: createId(),
          jsonrpc: '2.0',
          success: false,
          error: {
            code: -1,
            message: error instanceof Error ? error.message : 'Unknown error',
          },
        };
      }
    },
    [],
  );

  const service: LocalService = useMemo(() => {
    return {
      name: 'agentic-flow-control',
      tools: [
        {
          toolDefinition: {
            name: 'switch_assistant',
            description:
              'Switch control between assistants in the 2-agent workflow. Use this to delegate tasks to the appropriate assistant or return control to the supervisor.',
            inputSchema: createObjectSchema({
              properties: {
                from: createStringSchema({
                  description:
                    'The ID or name of the current assistant (who is making the switch)',
                }),
                to: createStringSchema({
                  description:
                    'The ID or name of the target assistant to switch to',
                }),
                query: createStringSchema({
                  description:
                    'The specific task, question, or instruction for the target assistant',
                }),
              },
              required: ['from', 'to', 'query'],
            }),
          },
          handler: handleSwitchAssistant,
        },
        {
          toolDefinition: {
            name: 'report_to_user',
            description:
              'Report the final results, conclusions, or status to the user when the task is complete or when important findings need to be communicated.',
            inputSchema: createObjectSchema({
              properties: {
                report: createStringSchema({
                  description:
                    'The comprehensive final report, conclusion, or status update for the user',
                }),
                isComplete: createStringSchema({
                  description:
                    'Whether the task has been fully completed. Use "true" for complete, "false" for progress update. Defaults to "true" if not specified.',
                }),
              },
              required: ['report'],
            }),
          },
          handler: handleReportToUser,
        },
      ],
    };
  }, [handleSwitchAssistant, handleReportToUser]);

  // 🎯 Worker의 공개 정보만 포함하는 System Prompt
  const superviserAssistant: Assistant = useMemo(() => {
    const workerInfo = getWorkerInfoSummary();

    return {
      name: 'Supervisor Assistant',
      id: SUPERVISER_ASSISTANT_ID,
      localServices: [service.name],
      createdAt: new Date(),
      updatedAt: new Date(),
      isDefault: false,
      mcpConfig: {},
      systemPrompt: `You are a supervisor assistant responsible for orchestrating a 2-agent workflow. Your role is to plan tasks systematically and coordinate with your worker assistant.

WORKER ASSISTANT INFORMATION:
${workerInfo}

WORKFLOW PROCESS:
1. Analyze the user's request thoroughly
2. Use 'switch_assistant' to delegate specific tasks to the worker assistant
3. When control returns to you (after worker completes their task), assess progress and determine next steps
4. Continue this coordination cycle until the task is complete
5. Use 'report_to_user' when you have a complete solution or final answer

2-AGENT COORDINATION:
- You are the PLANNER and COORDINATOR
- The worker assistant is the EXECUTOR
- Use 'switch_assistant' to hand off control with specific instructions
- When the worker completes their task, control will automatically return to you
- Maintain oversight of the overall workflow and progress

SWITCH ASSISTANT USAGE:
- from: Always use "${SUPERVISER_ASSISTANT_ID}" when you are switching
- to: Use the worker's ID or name when delegating tasks
- query: Provide clear, specific instructions for what the worker should do

GUIDELINES:
- Always think step-by-step and be methodical
- Consider the worker's capabilities when delegating tasks
- Use 'switch_assistant' to delegate execution while maintaining coordination
- Each switch should have a clear purpose and specific instructions
- Be thorough but efficient - avoid unnecessary handoffs
- Use 'report_to_user' only when you have a complete, actionable answer
- Include all relevant details and context in your final report
- Work collaboratively with the worker assistant to achieve the best results

Remember: This is a 2-agent system where you coordinate and the worker executes. Your systematic coordination combined with the worker's execution capabilities will provide comprehensive solutions to user requests.`,
    };
  }, [service.name, getWorkerInfoSummary]);

  // 🎯 자동 Assistant 전환 로직
  useEffect(() => {
    if (idle && messages.length > 0) {
      const lastMessage = messages[messages.length - 1];

      // 마지막 메시지가 supervisor에 의한 것이 아니고, reportToUser가 아닐 때
      const isFromSupervisor =
        lastMessage?.assistantId === SUPERVISER_ASSISTANT_ID;
      const isReportToUser = lastReportRef.current;

      if (!isFromSupervisor && !isReportToUser) {
        const currentAssistant = getCurrentAssistant();
        if (
          currentAssistant &&
          currentAssistant.id === SUPERVISER_ASSISTANT_ID
        ) {
          const workerAssistant = getWorkerInstance();
          const workerName = workerAssistant?.name || 'worker';

          // assistant 메시지에 tool_calls 포함 (ToolCaller가 자동으로 처리)
          const autoSwitchMessage: Message = {
            id: createId(),
            assistantId: SUPERVISER_ASSISTANT_ID,
            role: 'assistant',
            content:
              "I need to coordinate the next steps based on the worker's completion. Let me switch back to review progress and plan the next action.",
            tool_calls: [
              {
                id: createId(),
                type: 'function',
                function: {
                  name: 'switch_assistant',
                  arguments: JSON.stringify({
                    from: workerName,
                    to: SUPERVISER_ASSISTANT_ID,
                    query: 'Review worker completion and coordinate next steps',
                  }),
                },
              },
            ],
            sessionId: currentSession?.id || '',
            isStreaming: false,
          };

          submit([autoSwitchMessage]);
        } else {
          logger.info('[Agentic Flow] Auto-switching to supervisor assistant');

          // 리포트 플래그 리셋
          lastReportRef.current = false;

          // Supervisor로 전환
          const supervisorAssistant = assistants?.find(
            (assistant) => assistant.id === SUPERVISER_ASSISTANT_ID,
          );

          if (supervisorAssistant) {
            // 불필요한 delay 제거하고 즉시 supervisor로 전환
            setCurrentAssistant(supervisorAssistant);
            // ToolCaller 패턴에 맞게 tool_calls가 포함된 assistant 메시지를 submit
          }
        }
      } else {
        // 리포트 플래그 리셋 (다음 사이클을 위해)
        lastReportRef.current = false;
      }
    }
  }, [
    idle,
    messages,
    currentSession,
    setCurrentAssistant,
    submit,
    getWorkerInstance,
    assistants,
  ]);

  // 서비스 등록 및 정리
  useEffect(() => {
    try {
      logger.info('[Agentic Flow] Registering service:', service.name);
      registerService(service);
      registerEphemeralAssistant(superviserAssistant);
      setError(null);
    } catch (error) {
      logger.error('[Agentic Flow] Service registration error:', error);
      setError(
        error instanceof Error ? error.message : 'Service registration failed',
      );
    }

    return () => {
      if (!superviserAssistant.id) {
        toast.error(
          'Supervisor assistant ID가 존재하지 않아 서비스 정리를 건너뜁니다.',
        );
        return;
      }
      try {
        logger.info('[Agentic Flow] Unregistering service:', service.name);
        // Null check 개선
        const worker = currentSession?.assistants?.find(
          (assistant) => assistant.id !== SUPERVISER_ASSISTANT_ID,
        );
        if (worker) {
          setCurrentAssistant(worker);
        }
        unregisterService(service.name);
        unregisterEphemeralAssistant(superviserAssistant.id);
      } catch (error) {
        logger.error('[Agentic Flow] Service cleanup error:', error);
      }
    };
  }, [
    registerService,
    service,
    unregisterService,
    currentSession,
    registerEphemeralAssistant,
    unregisterEphemeralAssistant,
    superviserAssistant,
    setCurrentAssistant,
  ]);

  // 에러 로깅
  useEffect(() => {
    if (error) {
      logger.error('[Agentic Flow] Component error:', error);
    }
  }, [error]);

  return null;
}

export default SimpleAgenticFlow;
