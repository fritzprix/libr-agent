import { dbService } from '@/lib/db/service';
import {
  McpServerService,
  type IMcpServerService,
} from '@/lib/services/mcp-server-service';
import {
  AssistantService,
  type IAssistantService,
} from '@/lib/services/assistant-service';

export interface McpManagerServices {
  mcpService: IMcpServerService;
  assistantService: IAssistantService;
}

let cachedServices: {
  mcpService: IMcpServerService;
  assistantService: IAssistantService;
  agentHubUrl: string | undefined;
} | null = null;

export async function getServices(): Promise<McpManagerServices> {
  const agentHubUrlObj = await dbService.objects.read('agentHubUrl');
  const agentHubUrl = agentHubUrlObj?.value as string;

  if (cachedServices && cachedServices.agentHubUrl === agentHubUrl) {
    return {
      mcpService: cachedServices.mcpService,
      assistantService: cachedServices.assistantService,
    };
  }

  const mcpService = new McpServerService(agentHubUrl);
  const assistantService = new AssistantService(agentHubUrl);

  cachedServices = {
    mcpService,
    assistantService,
    agentHubUrl,
  };

  return { mcpService, assistantService };
}
