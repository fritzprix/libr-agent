import {
  LocalService,
  ServiceToolHandler,
  useLocalTools,
} from '@/context/LocalToolContext';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo } from 'react';

function GoogleDriveService() {
  const { registerService, unregisterService } = useLocalTools();

  const listRecentFiles: ServiceToolHandler<unknown> = useCallback(async () => {
    return {
      id: createId(),
      jsonrpc: '2.0',
      result: {
        content: [{ type: 'text', text: 'file1' }],
      },
    };
  }, []);
  const service: LocalService = useMemo(
    () => ({
      name: 'google-drive-service',
      tools: [
        {
          toolDefinition: {
            name: 'listRecentFiles',
            description: '',
            inputSchema: {
              type: 'object',
            },
          },
          handler: listRecentFiles,
        },
      ],
    }),
    [listRecentFiles],
  );
  useEffect(() => {
    registerService(service);
    return () => {
      unregisterService(service.name);
    };
  }, [registerService, unregisterService, service]);

  return null;
}

export { GoogleDriveService };
