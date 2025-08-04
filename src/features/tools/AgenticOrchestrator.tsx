import {
  LocalService,
  ServiceToolHandler,
  useLocalTools,
} from '@/context/LocalToolContext';
import { createObjectSchema } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo } from 'react';

const logger = getLogger('AgenticOrchestrator');
const SERVICE_NAME = 'simple-agentic-orchestrator';

interface ObserveProgressInput {
  observation: string;
}

function AgenticOrchestrator() {
  const { getAvailableServices, registerService, unregisterService } =
    useLocalTools();

  const observeProgress: ServiceToolHandler<ObserveProgressInput> = useCallback(
    async ({ observation }: ObserveProgressInput) => {
      logger.debug('Recording progress observation', { observation });

      return {
        id: createId(),
        jsonrpc: '2.0',
        result: {
          content: [
            {
              type: 'text',
              text: `📊 Progress: ${observation}`,
            },
          ],
        },
      };
    },
    [],
  );

  const service: LocalService = useMemo(
    () => ({
      name: SERVICE_NAME,
      tools: [
        {
          toolDefinition: {
            name: 'observeProgress',
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
          handler: observeProgress as ServiceToolHandler<unknown>,
        },
      ],
    }),
    [observeProgress],
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
