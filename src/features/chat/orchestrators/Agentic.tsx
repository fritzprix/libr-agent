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
import { Assistant } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

const logger = getLogger('Agentic.tsx');

interface PlanNextStepInputType {
  currentSituation: string;
  reasoning: string;
  nextAction: string;
}

interface ReportToUserInputType {
  report: string;
  isComplete?: string;
}

const SUPERVISER_ASSISTANT_ID = 'superviser-assistant';

export function SimpleAgenticFlow() {
  const [isFlowActive, setIsFlowActive] = useState(false);
  const [hasReportedToUser, setHasReportedToUser] = useState(false);
  const { current: currentSession } = useSessionContext();
  const [error, setError] = useState<string | null>(null);
  const flowStepCountRef = useRef(0);
  const maxStepsRef = useRef(10);

  const { submit, messages } = useChatContext();
  const { setCurrentAssistant, getCurrentAssistant } = useAssistantContext();
  const { idle, schedule } = useScheduler();
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

  const handlePlanNextStep = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      try {
        const { currentSituation, nextAction, reasoning } =
          args as PlanNextStepInputType;

        if (!currentSituation || !nextAction || !reasoning) {
          throw new Error(
            'Missing required fields: currentSituation, nextAction, reasoning',
          );
        }

        flowStepCountRef.current += 1;
        logger.info(
          `[Agentic Flow] Step ${flowStepCountRef.current}: Planning next action`,
        );
        logger.info(`- Current Situation: ${currentSituation}`);
        logger.info(`- Next Action: ${nextAction}`);
        logger.info(`- Reasoning: ${reasoning}`);

        // 🎯 단일 Worker에게 작업 위임
        const workerAssistant = getWorkerInstance();

        if (workerAssistant) {
          logger.info(
            `[Agentic Flow] Delegating execution to worker: ${workerAssistant.name}`,
          );

          // Worker로 전환 (응답 완료 후)
          setTimeout(() => {
            setCurrentAssistant(workerAssistant);
          }, 100);
        } else {
          logger.error('[Agentic Flow] No worker assistant found in session');
        }

        // 최대 단계 수 체크
        if (flowStepCountRef.current >= maxStepsRef.current) {
          logger.warn(
            '[Agentic Flow] Maximum steps reached, suggesting completion',
          );
          return {
            id: createId(),
            jsonrpc: '2.0',
            success: true,
            result: {
              content: [
                {
                  type: 'text',
                  text: `Step ${flowStepCountRef.current} completed. Maximum steps reached. Please consider using report_to_user to finish the task.`,
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
                text: `Step ${flowStepCountRef.current} completed. Next action planned: ${nextAction}. Delegating execution to worker assistant.`,
              },
            ],
          },
        };
      } catch (error) {
        logger.error('[Agentic Flow] Plan next step error:', error);
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
        const isComplete = rawArgs.isComplete !== 'false';

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

        setHasReportedToUser(true);
        setIsFlowActive(false);
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
            name: 'plan_next_step',
            description:
              'Plan the next step in solving the current task. Analyze the current situation and determine the most logical next action to take. After planning, execution will be delegated to the worker assistant.',
            inputSchema: createObjectSchema({
              properties: {
                currentSituation: createStringSchema({
                  description:
                    'Description of the current state and what has been accomplished so far',
                }),
                nextAction: createStringSchema({
                  description: 'The specific next action to take',
                }),
                reasoning: createStringSchema({
                  description:
                    'The reasoning behind why this next action is the best choice',
                }),
              },
              required: ['currentSituation', 'nextAction', 'reasoning'],
            }),
          },
          handler: handlePlanNextStep,
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
  }, [handlePlanNextStep, handleReportToUser]);

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
      systemPrompt: `You are a supervisor assistant responsible for orchestrating a 2-agent workflow. Your role is to plan tasks systematically and delegate execution to your worker assistant.

WORKER ASSISTANT INFORMATION:
${workerInfo}

WORKFLOW PROCESS:
1. Analyze the user's request thoroughly
2. Use 'plan_next_step' to break down the task into logical steps
3. After each planning step, execution will be automatically delegated to the worker assistant
4. When control returns to you (after worker completes their task), assess progress and plan the next step
5. Continue this cycle until the task is complete
6. Use 'report_to_user' when you have a complete solution or final answer

2-AGENT COORDINATION:
- You are the PLANNER and COORDINATOR
- The worker assistant is the EXECUTOR
- After each 'plan_next_step', the worker will automatically take over to execute the planned action
- When the worker completes their task, control will return to you for the next planning cycle
- Maintain oversight of the overall workflow and progress

GUIDELINES:
- Always think step-by-step and be methodical
- Consider the worker's capabilities (based on their description, services, and MCP capabilities) when planning tasks
- Use 'plan_next_step' to document your progress and reasoning before delegating
- Each step should move closer to solving the user's request
- Be thorough but efficient - avoid unnecessary steps
- Use 'report_to_user' only when you have a complete, actionable answer for the user
- Include all relevant details and context in your final report
- Work collaboratively with the worker assistant to achieve the best results

Remember: This is a 2-agent system where you plan and the worker executes. Your systematic planning combined with the worker's execution capabilities will provide comprehensive solutions to user requests.`,
    };
  }, [service.name, getWorkerInfoSummary]);

  // 핵심 흐름 제어 로직
  useEffect(() => {
    if (!idle || hasReportedToUser) {
      return;
    }

    const lastAssistantMessage = [...messages]
      .reverse()
      .find((m) => m.role === 'assistant');

    // report_to_user 호출 감지로 흐름 종료
    const hasCalledReportToUser = lastAssistantMessage?.tool_calls?.some(
      (tc) => tc.function.name === 'report_to_user',
    );

    if (hasCalledReportToUser) {
      logger.info('[Agentic Flow] Detected report_to_user call, stopping flow');
      setHasReportedToUser(true);
      setIsFlowActive(false);
      return;
    }

    const current = getCurrentAssistant();

    if (current && current.id === SUPERVISER_ASSISTANT_ID) {
      if (!isFlowActive) {
        setIsFlowActive(true);
      }

      logger.info('[Agentic Flow] Scheduling next step submission');
      schedule(async () => {
        try {
          await submit();
        } catch (error) {
          logger.error('[Agentic Flow] Submit error:', error);
          setError(error instanceof Error ? error.message : 'Submit failed');
          setIsFlowActive(false);
        }
      });
    } else {
      // 🎯 마지막 메시지가 supervisor 것이 아닐 때만 supervisor로 전환
      const isLastMessageFromSupervisor =
        lastAssistantMessage?.tool_calls?.some(
          (tc) =>
            tc.function.name === 'plan_next_step' ||
            tc.function.name === 'report_to_user',
        );

      if (!isLastMessageFromSupervisor) {
        logger.info(
          '[Agentic Flow] Activating supervisor assistant (last message not from supervisor)',
        );
        setCurrentAssistant(superviserAssistant);
        setHasReportedToUser(false);
        flowStepCountRef.current = 0;
      } else {
        logger.info(
          '[Agentic Flow] Last message was from supervisor, skipping activation',
        );
      }
    }
  }, [
    idle,
    messages,
    hasReportedToUser,
    isFlowActive,
    superviserAssistant,
    getCurrentAssistant,
    setCurrentAssistant,
    schedule,
    submit,
  ]);

  // 서비스 등록 및 정리
  useEffect(() => {
    try {
      logger.info('[Agentic Flow] Registering service:', service.name);
      registerService(service);
      setError(null);
    } catch (error) {
      logger.error('[Agentic Flow] Service registration error:', error);
      setError(
        error instanceof Error ? error.message : 'Service registration failed',
      );
    }

    return () => {
      try {
        logger.info('[Agentic Flow] Unregistering service:', service.name);
        const worker = currentSession?.assistants[0];
        if (worker) {
          setCurrentAssistant(worker);
        }

        unregisterService(service.name);
      } catch (error) {
        logger.error('[Agentic Flow] Service cleanup error:', error);
      }
    };
  }, [registerService, service, unregisterService, currentSession]);

  // 에러 로깅
  useEffect(() => {
    if (error) {
      logger.error('[Agentic Flow] Component error:', error);
    }
  }, [error]);

  return null;
}

export default SimpleAgenticFlow;
