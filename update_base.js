const fs = require('fs');
const path = require('path');

const filePath = path.join('src', 'lib', 'ai-service', 'base-service.ts');
let content = fs.readFileSync(filePath, 'utf8');

// Update class declaration
content = content.replace(
  'export abstract class BaseAIService implements IAIService {',
  'export abstract class BaseAIService<TProviderMessage = unknown, TProviderTool = unknown> implements IAIService {',
);

// Update prepareStreamChat return type
content = content.replace('tools?: unknown[];', 'tools?: TProviderTool[];');

// Remove extractMediaContent and convertMessagesTemplate, listModels stays
content = content.replace(
  /\/\*\*\s*\n\s*\* Extracts image and audio items[\s\S]*?protected convertMessagesTemplate\([\s\S]*?return result;\n  }/,
  '',
);

// Update convertTools and abstract methods
content = content.replace(
  /abstract convertTools\(mcpTools: MCPTool\[\]\): unknown\[\];\s*\/\*\*[\s\S]*?protected abstract convertSingleMessage\(message: Message\): unknown;/m,
  `abstract convertTools(mcpTools: MCPTool[]): TProviderTool[];

  /**
   * Converts an array of standard \`Message\` objects into the provider-specific format.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of provider-specific message objects.
   * @protected
   * @abstract
   */
  protected abstract convertMessages(messages: Message[], systemPrompt?: string): TProviderMessage[];`,
);

fs.writeFileSync(filePath, content);
console.log('base-service.ts updated');
