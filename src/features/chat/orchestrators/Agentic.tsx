import {
  LocalService,
  MCPResponse,
  useLocalTools,
} from '@/context/LocalToolContext';
import { useChatContext } from '@/hooks/use-chat';
import { createObjectSchema, createStringSchema } from '@/lib/tauri-mcp-client';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo } from 'react';

interface RecursionInputType {
  reason: string;
}

interface ReportToUserInputType {
  report: string;
}

export function SimpleAgenticFlow() {

  const handleRecursion = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { reason } = args as RecursionInputType;

      return {
        id: createId(),
        jsonrpc: '2.0',
        success: true,
        result: {
          content: [{ type: 'text', text: reason }],
        },
      };
    },
    [],
  );

  const handleReportToUser = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const { report } = args as ReportToUserInputType;
      return {
        id: createId(),
        jsonrpc: '2.0',
        success: true,
        result: {
          content: [
            {
              type: 'text',
              text: `Report: ${report}`,
            },
          ],
        },
      };
    },
    [],
  );

  const service: LocalService = useMemo(() => {
    return {
      name: 'agentic-flow-control',
      tools: [
        {
          toolDefinition: {
            name: 'think_recursively',
            description: 'Continue thinking through the problem with reasoning',
            inputSchema: createObjectSchema({
              properties: {
                reason: createStringSchema({
                  description: 'The reasoning or thought process',
                }),
              },
              required: ['reason'],
            }),
          },
          handler: handleRecursion,
        },
        {
          toolDefinition: {
            name: 'report_to_user',
            description: 'Report final results or status to the user',
            inputSchema: createObjectSchema({
              properties: {
                report: createStringSchema({
                  description: 'The report to send to the user',
                }),
              },
              required: ['report'],
            }),
          },
          handler: handleReportToUser,
        },
      ],
    };
  }, [handleRecursion, handleReportToUser]);

  const { unregisterService, registerService } = useLocalTools();


  useEffect(() => {
    registerService(service);
    return () => {
      unregisterService(service.name);
    };
  }, [registerService, service, unregisterService]);

  return null;
}

export default SimpleAgenticFlow;
