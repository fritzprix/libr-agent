import {
  LocalService,
  ServiceToolHandler,
  useLocalTools,
} from '@/context/LocalToolContext';
import { createObjectSchema } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo, useRef } from 'react';

const logger = getLogger('AgenticOrchestrator');
const SERVICE_NAME = 'simple-agentic-orchestrator';

interface ObserveInput {
  observation: string;
}

interface PlanInput {
  plan: string;
}

interface SetGoalInput {
  goal: string;
}

interface HaltInput {
  prompt: string;
  isFinished: boolean;
}

interface AgenticContext {
  goal?: string;
  plan?: string;
  observation?: string;
  status?: 'active' | 'completed' | 'halted';
  haltReason?: string;
  timestamp?: string;
}

function AgenticOrchestrator() {
  const { getAvailableServices, registerService, unregisterService } =
    useLocalTools();
  const agenticContextRef = useRef<AgenticContext>({});

  const observe: ServiceToolHandler<ObserveInput> = useCallback(
    async ({ observation }: ObserveInput) => {
      logger.debug('Recording progress observation', { observation });
      agenticContextRef.current = {
        ...agenticContextRef.current,
        observation,
        timestamp: new Date().toISOString(),
      };

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(agenticContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  const plan: ServiceToolHandler<PlanInput> = useCallback(
    async ({ plan }: PlanInput) => {
      logger.info('Setting execution plan', { plan });
      agenticContextRef.current = {
        ...agenticContextRef.current,
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
              text: JSON.stringify(agenticContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  const setGoal: ServiceToolHandler<SetGoalInput> = useCallback(
    async ({ goal }: SetGoalInput) => {
      logger.info('Setting agentic goal', { goal });
      agenticContextRef.current = {
        ...agenticContextRef.current,
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
              text: JSON.stringify(agenticContextRef.current),
            },
          ],
        },
      };
    },
    [],
  );

  const halt: ServiceToolHandler<HaltInput> = useCallback(
    async ({ prompt, isFinished }: HaltInput) => {
      logger.info('Task halted', { prompt, isFinished });

      // 현재 컨텍스트에 완료 상태 추가
      const finalContext: AgenticContext = {
        ...agenticContextRef.current,
        status: isFinished ? 'completed' : 'halted',
        haltReason: prompt,
        timestamp: new Date().toISOString(),
      };

      agenticContextRef.current = finalContext;

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

  const getContext: ServiceToolHandler<Record<string, never>> =
    useCallback(async () => {
      logger.debug('Retrieving current agentic context');
      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: JSON.stringify(agenticContextRef.current),
            },
          ],
        },
      };
    }, []);

  const clearContext: ServiceToolHandler<Record<string, never>> =
    useCallback(async () => {
      logger.info('Clearing agentic context');
      const previousContext = { ...agenticContextRef.current };
      agenticContextRef.current = {};

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
    }, []);

  const service: LocalService = useMemo(
    () => ({
      name: SERVICE_NAME,
      tools: [
        {
          toolDefinition: {
            name: 'observe',
            description: 'Record and observe progress during task execution',
            inputSchema: createObjectSchema({
              description: 'Input for recording progress observations',
              properties: {
                observation: {
                  type: 'string',
                  description: 'The progress observation to record',
                },
              },
              required: ['observation'],
            }),
          },
          handler: observe as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'plan',
            description:
              'Create and set a detailed execution plan for the current task',
            inputSchema: createObjectSchema({
              description: 'Input for setting task execution plan',
              properties: {
                plan: {
                  type: 'string',
                  description: 'Detailed step-by-step plan for task execution',
                },
              },
              required: ['plan'],
            }),
          },
          handler: plan as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'setGoal',
            description: 'Set the main goal or objective for the agentic task',
            inputSchema: createObjectSchema({
              description: 'Input for setting task goal',
              properties: {
                goal: {
                  type: 'string',
                  description: 'The main goal or objective to achieve',
                },
              },
              required: ['goal'],
            }),
          },
          handler: setGoal as ServiceToolHandler<unknown>,
        },
        {
          toolDefinition: {
            name: 'halt',
            description: 'Stop or complete the current agentic task execution',
            inputSchema: createObjectSchema({
              description: 'Input for halting task execution',
              properties: {
                prompt: {
                  type: 'string',
                  description: 'Reason for halting or completion message',
                },
                isFinished: {
                  type: 'boolean',
                  description:
                    'Whether the task is completed (true) or halted due to error/interruption (false)',
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
            description:
              'Retrieve the current agentic context including goal, plan, and observations',
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
            description: 'Clear all agentic context and reset to initial state',
            inputSchema: createObjectSchema({
              description: 'No input required - clears all context',
              properties: {},
            }),
          },
          handler: clearContext as ServiceToolHandler<unknown>,
        },
      ],
    }),
    [observe, plan, setGoal, halt, getContext, clearContext],
  );

  useEffect(() => {
    const services = getAvailableServices();
    if (!services.some((s) => s === SERVICE_NAME)) {
      logger.info('Registering agentic orchestrator service');
      registerService(service);
      return () => {
        logger.info('Unregistering agentic orchestrator service');
        unregisterService(service.name);
      };
    }
  }, [getAvailableServices, registerService, unregisterService, service]);

  return null;
}

export { AgenticOrchestrator };
