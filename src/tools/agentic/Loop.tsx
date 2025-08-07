import {
  LocalService,
  ServiceToolHandler,
  useLocalTools,
} from '@/context/LocalToolContext';
import { useAssistantExtension } from '@/context/AssistantExtensionContext';
import { useChatContext } from '@/context/ChatContext';
import { useScheduler } from '@/context/SchedulerContext';
import { createObjectSchema } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo, useRef } from 'react';

// LOOP 프레임워크에 맞게 로거 및 서비스 이름 수정
const logger = getLogger('LoopFramework');
const SERVICE_NAME = 'loop-framework';

const AGENTIC_LOOP_PROMPT = `
You are an agent operating under the LOOP framework.
Use the following tools step-by-step to solve complex problems:
learn, observe, orient, plan, halt, getContext, clearContext.
Call each tool appropriately and deliver the final result using 'halt' when the task is complete.
`;

// LOOP 사이클에 맞는 입력 인터페이스 정의
interface LearnInput {
  learning: string;
}

interface ObserveInput {
  observation: string;
}

interface OrientInput {
  goal: string;
}

interface PlanInput {
  plan: string;
}

interface HaltInput {
  prompt: string;
  isFinished: boolean;
}

// LOOP 프레임워크의 상태를 관리하는 컨텍스트 인터페이스
interface LoopContext {
  goal?: string;
  plan?: string;
  learnings?: string[]; // 학습된 내용을 배열로 관리
  lastObservation?: string; // 마지막 관찰 결과
  status?: 'active' | 'completed' | 'halted';
  haltReason?: string;
  timestamp?: string;
}

/**
 * Learn-Observe-Orient-Plan (LOOP) 사이클을 기반으로
 * 에이전트의 작업을 조율하는 프레임워크 컴포넌트입니다.
 */
function LoopFramework() {
  const { getAvailableServices, registerService, unregisterService } =
    useLocalTools();
  const { registerExtension, unregisterExtension } = useAssistantExtension();
  const { messages, submit } = useChatContext();
  const { idle } = useScheduler();
  const loopContextRef = useRef<LoopContext>({});
  const lastMessageRef = useRef<string|null>(null);

  // Learn: 관찰과 행동으로부터 얻은 교훈이나 결론을 기록합니다.
  const learn: ServiceToolHandler<LearnInput> = useCallback(
    async ({ learning }: LearnInput) => {
      logger.debug('Recording a learning', { learning });
      const currentLearnings = loopContextRef.current.learnings || [];
      loopContextRef.current = {
        ...loopContextRef.current,
        learnings: [...currentLearnings, learning],
        timestamp: new Date().toISOString(),
      };

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(loopContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  // Observe: 환경이나 태스크 실행 결과에 대한 관찰 내용을 기록합니다.
  const observe: ServiceToolHandler<ObserveInput> = useCallback(
    async ({ observation }: ObserveInput) => {
      logger.debug('Recording an observation', { observation });
      loopContextRef.current = {
        ...loopContextRef.current,
        lastObservation: observation,
        timestamp: new Date().toISOString(),
      };

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(loopContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  // Orient: 에이전트가 나아갈 방향, 즉 최종 목표를 설정합니다.
  const orient: ServiceToolHandler<OrientInput> = useCallback(
    async ({ goal }: OrientInput) => {
      logger.info('Orienting towards a goal', { goal });
      // 새로운 목표가 설정되면 컨텍스트를 초기화하여 새로운 LOOP 시작
      loopContextRef.current = {
        learnings: [],
        goal,
        status: 'active',
        timestamp: new Date().toISOString(),
      };

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(loopContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  // Plan: 설정된 목표를 달성하기 위한 구체적인 실행 계획을 수립합니다.
  const plan: ServiceToolHandler<PlanInput> = useCallback(
    async ({ plan }: PlanInput) => {
      logger.info('Setting execution plan', { plan });
      loopContextRef.current = {
        ...loopContextRef.current,
        plan,
        timestamp: new Date().toISOString(),
      };

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(loopContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  // Halt: 현재 진행 중인 LOOP 사이클을 중단하거나 완료 처리합니다.
  const halt: ServiceToolHandler<HaltInput> = useCallback(
    async ({ prompt, isFinished }: HaltInput) => {
      logger.info('Task halted', { prompt, isFinished });

      const finalContext: LoopContext = {
        ...loopContextRef.current,
        status: isFinished ? 'completed' : 'halted',
        haltReason: prompt,
        timestamp: new Date().toISOString(),
      };

      loopContextRef.current = finalContext;

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(finalContext),
            },
          ],
        },
      };
    },
    [],
  );

  // getContext: 현재 LOOP 컨텍스트 전체를 조회합니다.
  const getContext: ServiceToolHandler<Record<string, never>> = useCallback(
    async () => {
      logger.debug('Retrieving current LOOP context');
      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(loopContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  // clearContext: 현재 LOOP 컨텍스트를 초기화합니다.
  const clearContext: ServiceToolHandler<Record<string, never>> = useCallback(
    async () => {
      logger.info('Clearing LOOP context');
      const previousContext = { ...loopContextRef.current };
      loopContextRef.current = {};
      logger.debug('Context cleared', { previousContext });

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: 'Context cleared successfully',
            },
          ],
        },
      };
    },
    [],
  );

  // LOOP 프레임워크가 제공하는 서비스와 도구들을 정의합니다.
  const service: LocalService = useMemo(
    () => ({
      name: SERVICE_NAME,
      tools: [
        // LOOP 사이클 순서에 맞춰 도구 정렬
        {
          toolDefinition: {
            name: 'learn',
            description:
              'Record a synthesized learning from previous observations and actions. Part of the LOOP cycle.',
            inputSchema: createObjectSchema({
              description: 'Input for recording a learning',
              properties: {
                learning: {
                  type: 'string',
                  description:
                    'The new insight or conclusion to record',
                },
              },
              required: ['learning'],
            }),
          },
          handler: learn as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'observe',
            description:
              'Record an observation about the environment or task state. Part of the LOOP cycle.',
            inputSchema: createObjectSchema({
              description: 'Input for recording an observation',
              properties: {
                observation: {
                  type: 'string',
                  description: 'The observation to record',
                },
              },
              required: ['observation'],
            }),
          },
          handler: observe as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'orient',
            description:
              'Set the primary goal or objective, orienting the agent. Part of the LOOP cycle.',
            inputSchema: createObjectSchema({
              description: 'Input for orienting towards a goal',
              properties: {
                goal: {
                  type: 'string',
                  description: 'The main goal or objective to achieve',
                },
              },
              required: ['goal'],
            }),
          },
          handler: orient as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'plan',
            description:
              'Create a step-by-step plan to achieve the goal. Part of the LOOP cycle.',
            inputSchema: createObjectSchema({
              description: 'Input for setting the execution plan',
              properties: {
                plan: {
                  type: 'string',
                  description: 'Detailed step-by-step plan for execution',
                },
              },
              required: ['plan'],
            }),
          },
          handler: plan as ServiceToolHandler<unknown>,
        },
        // --- 메타 도구들 ---
        {
          toolDefinition: {
            name: 'halt',
            description: 'Stop or complete the current LOOP cycle execution',
            inputSchema: createObjectSchema({
              description: 'Input for halting execution',
              properties: {
                prompt: {
                  type: 'string',
                  description: 'Reason for halting or completion message',
                },
                isFinished: {
                  type: 'boolean',
                  description:
                    'Whether the task is completed (true) or halted (false)',
                },
              },
              required: ['prompt', 'isFinished'],
            }),
          },
          handler: halt as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'getContext',
            description: 'Retrieve the current LOOP context',
            inputSchema: createObjectSchema({
              description: 'No input required - returns current context',
              properties: {},
            }),
          },
          handler: getContext as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'clearContext',
            description: 'Clear all LOOP context and reset to initial state',
            inputSchema: createObjectSchema({
              description: 'No input required - clears all context',
              properties: {},
            }),
          },
          handler: clearContext as ServiceToolHandler<unknown>,
        },
      ],
    }),
    [learn, observe, orient, plan, halt, getContext, clearContext],
  );

  useEffect(() => {
    const services = getAvailableServices();
    if (!services.some((s) => s === SERVICE_NAME)) {
      logger.info('Registering LOOP Framework service');
      registerService(service);
      return () => {
        logger.info('Unregistering LOOP Framework service');
        unregisterService(service.name);
      };
    }
  }, [getAvailableServices, registerService, unregisterService, service]);

  // Register LOOP extension with AssistantExtensionContext
  useEffect(() => {
    const agenticExtension = {
      systemPrompts: [AGENTIC_LOOP_PROMPT],
      services: [
        {
          type: 'native' as const,
          service: service,
        },
      ],
    };

    logger.info('Registering LOOP framework extension');
    registerExtension('agentic-loop', agenticExtension);

    return () => {
      unregisterExtension('agentic-loop');
      logger.info('Unregistered LOOP framework extension');
    };
  }, [registerExtension, unregisterExtension, service]);

  // Auto-submit when idle and not halted
  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (idle &&
      lastMessage &&
      lastMessage.isStreaming === false &&
      lastMessage.function_call?.name !== 'halt' &&
      lastMessageRef.current !== lastMessage.id
    ) {
      logger.debug('Auto-submitting message in LOOP Framework', { lastMessage });
      const id = createId();
      lastMessageRef.current = lastMessage.id;
      submit([{
        id,
        sessionId: '', // Will be set by submit function
        role: 'system',
        content: 'Reflect on your current LOOP context and decide the next step. Use getContext to review your current state, then proceed with the appropriate LOOP action (learn, observe, orient, plan, or halt).',
        isStreaming: false,
      }]);
    }
  }, [idle, messages, submit]);

  return null;
}

export { LoopFramework };
