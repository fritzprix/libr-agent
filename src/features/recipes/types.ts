import type { ExecutionMode } from '@/lib/generated/execution-mode';

export interface SolutionRecipe {
  id: string;
  titleKey: string;
  defaultTitle: string;
  descKey: string;
  defaultDesc: string;
  badge: string;
  executionMode: ExecutionMode;
  requiredMcpPresets: Array<{ presetName: string }>;
  assistantTemplate: {
    name: string;
    description: string;
    systemPrompt: string;
    allowedBuiltInServiceAliases: string[];
    mcpServerIds?: string[];
  };
  scheduledTaskTemplate: {
    name: string;
    cronExpression: string;
    message: string;
    executionMode: ExecutionMode;
  };
  testRunPrompt: string;
}
