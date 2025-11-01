/**
 * UI Tools Server - Built-in MCP server for user interaction
 *
 * Provides tools for creating interactive UI elements that communicate back to the agent:
 * - prompt_user: Display interactive prompts (text input, select, multiselect)
 * - reply_prompt: Receive user responses from prompt UI
 * - visualize_data: Create simple data visualizations (bar/line charts)
 * - wait_for_user_resume: Display wait UI with continue button
 * - resume_from_wait: Resume from wait state
 */

import {
  createMCPStructuredResponse,
  createMCPTextResponse,
  createMCPStructuredMultipartResponse,
} from '@/lib/mcp-response-utils';
import type {
  MCPResponse,
  WebMCPServer,
  MCPContent,
  ServiceInfo,
} from '@/lib/mcp-types';
import type { UIResource } from '@mcp-ui/server';
import { uiToolsSchema } from './tools.ts';
import {
  escapeHtml,
  createUiResourceWithServiceInfo,
  renderTemplate,
} from '../../utils/ui-utils';

// Import templates
import waitTemplate from './templates/wait.hbs?raw';
import textPromptTemplate from './templates/text-prompt.hbs?raw';
import selectPromptTemplate from './templates/select-prompt.hbs?raw';
import barChartTemplate from './templates/bar-chart.hbs?raw';
import lineChartTemplate from './templates/line-chart.hbs?raw';

let nextMessageId = 1;

// Store for tracking prompt states
interface PromptState {
  messageId: string;
  prompt: string;
  type: 'text' | 'select' | 'multiselect';
  options?: string[];
  createdAt: number;
}

const activePrompts = new Map<string, PromptState>();

/**
 * Format duration in human-readable format
 */
function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

/**
 * Create a UIResource with proper serviceInfo for routing
 */
function createUiResourceFromHtml(
  html: string,
  uri: `ui://${string}`,
): UIResource & { serviceInfo?: ServiceInfo } {
  return createUiResourceWithServiceInfo({
    html,
    serverName: 'ui',
    toolName: '',
    uri,
  });
}

/**
 * Build wait HTML from template
 * Note: Handlebars automatically escapes {{message}} and {{contextHtml}}
 */
function buildWaitHtml(
  message: string,
  resumeInstruction: string,
  startedAt: string,
): string {
  const context = { resumeInstruction, startedAt };
  const serializedContext = JSON.stringify(context).replace(
    /<\/(script)>/gi,
    '\\u003C/$1',
  );
  const htmlEscapedContext = escapeHtml(JSON.stringify(context));

  return renderTemplate(waitTemplate, {
    message,
    contextHtml: htmlEscapedContext,
    contextJson: serializedContext,
  });
}

/**
 * Build text prompt HTML from template
 * Note: Handlebars automatically escapes {{prompt}} and {{messageId}}
 */
function buildTextPromptHtml(messageId: string, prompt: string): string {
  return renderTemplate(textPromptTemplate, {
    prompt,
    messageId,
  });
}

/**
 * Build select prompt HTML from template
 * Note: Handlebars automatically escapes {{prompt}} and {{messageId}}
 * HTML in {{{optionsHtml}}} is not escaped (triple braces)
 */
function buildSelectPromptHtml(
  messageId: string,
  prompt: string,
  options: string[],
  multiselect: boolean,
): string {
  const inputType = multiselect ? 'checkbox' : 'radio';
  const optionsHtml = options
    .map((opt, idx) => {
      const escapedOpt = escapeHtml(opt);
      return `
      <label class="option-item">
        <input type="${inputType}" name="option" value="${idx}" class="option-input">
        <span class="option-label">${escapedOpt}</span>
      </label>`;
    })
    .join('');

  return renderTemplate(selectPromptTemplate, {
    prompt,
    messageId,
    optionsHtml,
    multiselect: multiselect ? 'true' : 'false',
    optionsJson: JSON.stringify(options),
  });
}

/**
 * Build bar chart HTML from template
 */
function buildBarChartHtml(
  data: Array<{ label: string; value: number }>,
): string {
  const maxValue = Math.max(...data.map((d) => d.value), 1);
  const chartHeight = 300;
  const barWidth = Math.max(40, Math.min(80, 400 / data.length));

  const bars = data
    .map((item, idx) => {
      const barHeight = (item.value / maxValue) * (chartHeight - 40);
      const x = idx * (barWidth + 10) + 50;
      const y = chartHeight - barHeight - 20;
      const escapedLabel = escapeHtml(item.label);

      return `
        <g>
          <rect x="${x}" y="${y}" width="${barWidth}" height="${barHeight}"
                fill="#3b82f6" rx="4" />
          <text x="${x + barWidth / 2}" y="${chartHeight - 5}"
                text-anchor="middle" font-size="12" fill="#6b7280">
            ${escapedLabel}
          </text>
          <text x="${x + barWidth / 2}" y="${y - 5}"
                text-anchor="middle" font-size="12" font-weight="600" fill="#1f2937">
            ${item.value}
          </text>
        </g>`;
    })
    .join('');

  const svgWidth = data.length * (barWidth + 10) + 100;
  const svgHeight = chartHeight + 20;

  return renderTemplate(barChartTemplate, {
    svgWidth: String(svgWidth),
    svgHeight: String(svgHeight),
    barsHtml: bars,
  });
}

/**
 * Build line chart HTML from template
 */
function buildLineChartHtml(
  data: Array<{ label: string; value: number }>,
): string {
  const maxValue = Math.max(...data.map((d) => d.value), 1);
  const chartHeight = 300;

  const points = data
    .map((item, idx) => {
      const x = (idx / (data.length - 1)) * 500 + 50;
      const y = chartHeight - (item.value / maxValue) * (chartHeight - 40) - 20;
      return `${x},${y}`;
    })
    .join(' ');

  const labels = data
    .map((item, idx) => {
      const x = (idx / (data.length - 1)) * 500 + 50;
      const y = chartHeight - (item.value / maxValue) * (chartHeight - 40) - 20;
      const escapedLabel = escapeHtml(item.label);
      return `
        <text x="${x}" y="${chartHeight - 5}"
              text-anchor="middle" font-size="12" fill="#6b7280">
          ${escapedLabel}
        </text>
        <circle cx="${x}" cy="${y}"
                r="4" fill="#3b82f6" />
        <text x="${x}" y="${y - 10}"
              text-anchor="middle" font-size="12" font-weight="600" fill="#1f2937">
          ${item.value}
        </text>`;
    })
    .join('');

  const svgHeight = chartHeight + 20;

  return renderTemplate(lineChartTemplate, {
    svgHeight: String(svgHeight),
    points,
    labelsHtml: labels,
  });
}

const uiTools: WebMCPServer = {
  name: 'ui',
  version: '0.1.0',
  description:
    'Built-in UI interaction tools for user prompts and visualizations',
  tools: uiToolsSchema,

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    try {
      switch (name) {
        case 'prompt_user': {
          const prompt = String(a.prompt || '');
          const type = String(a.type || 'text') as
            | 'text'
            | 'select'
            | 'multiselect';
          const options = (a.options as string[]) || [];

          if (!prompt) {
            return createMCPTextResponse('Prompt text is required');
          }

          if (
            (type === 'select' || type === 'multiselect') &&
            options.length === 0
          ) {
            return createMCPTextResponse(
              `Options are required for ${type} prompts`,
            );
          }

          // Generate unique message ID
          const messageId = `ui-${Date.now()}-${nextMessageId++}`;

          // Store prompt state
          activePrompts.set(messageId, {
            messageId,
            prompt,
            type,
            options: type !== 'text' ? options : undefined,
            createdAt: Date.now(),
          });

          // Generate HTML based on type
          let html: string;
          if (type === 'text') {
            html = buildTextPromptHtml(messageId, prompt);
          } else {
            html = buildSelectPromptHtml(
              messageId,
              prompt,
              options,
              type === 'multiselect',
            );
          }

          // Create UI resource
          const uiResource = createUiResourceFromHtml(
            html,
            `ui://prompts/${messageId}`,
          );

          // Create text content
          const textContent: MCPContent = {
            type: 'text',
            text: `📋 User prompt created (ID: ${messageId})\nType: ${type}\nPrompt: ${prompt}`,
          } as MCPContent;

          // Return multipart response
          return createMCPStructuredMultipartResponse(
            [textContent, uiResource],
            {
              messageId,
              type,
              prompt,
              options: type !== 'text' ? options : undefined,
              status: 'awaiting_response',
            },
          );
        }

        case 'reply_prompt': {
          const messageId = String(a.messageId || '');
          const answer = a.answer;
          const cancelled = Boolean(a.cancelled);

          if (!messageId) {
            return createMCPTextResponse('Message ID is required');
          }

          // Retrieve prompt state
          const promptState = activePrompts.get(messageId);
          if (!promptState) {
            return createMCPTextResponse(
              `Unknown prompt ID: ${messageId}. The prompt may have expired or already been answered.`,
            );
          }

          // Remove from active prompts
          activePrompts.delete(messageId);

          if (cancelled) {
            return createMCPStructuredResponse(
              `❌ User cancelled the prompt: "${promptState.prompt}"`,
              {
                messageId,
                cancelled: true,
                prompt: promptState.prompt,
                answer: null,
                timestamp: new Date().toISOString(),
              },
            );
          }

          // Format answer for display
          let answerText: string;
          if (Array.isArray(answer)) {
            answerText = answer.join(', ');
          } else if (answer === null || answer === undefined) {
            answerText = '(no answer)';
          } else {
            answerText = String(answer);
          }

          return createMCPStructuredResponse(
            `✅ User responded to: "${promptState.prompt}"\nAnswer: ${answerText}`,
            {
              messageId,
              prompt: promptState.prompt,
              answer,
              timestamp: new Date().toISOString(),
            },
          );
        }

        case 'visualize_data': {
          const type = String(a.type || 'bar') as 'bar' | 'line';
          const data =
            (a.data as Array<{ label: string; value: number }>) || [];
          const title = String(a.title || '');

          if (data.length === 0) {
            return createMCPTextResponse(
              'Data array is required and cannot be empty',
            );
          }

          // Validate data format
          for (const item of data) {
            if (!item.label || typeof item.value !== 'number') {
              return createMCPTextResponse(
                'Each data item must have a "label" (string) and "value" (number)',
              );
            }
          }

          // Generate visualization HTML
          const html =
            type === 'bar' ? buildBarChartHtml(data) : buildLineChartHtml(data);

          // Create UI resource
          const uiResource = createUiResourceFromHtml(
            html,
            `ui://visualizations/${type}-${Date.now()}`,
          );

          // Create text content
          const summary = data.map((d) => `${d.label}: ${d.value}`).join(', ');
          const textContent: MCPContent = {
            type: 'text',
            text: `📊 ${type.toUpperCase()} Chart${title ? `: ${title}` : ''}\nData: ${summary}`,
          } as MCPContent;

          // Return multipart response
          return createMCPStructuredMultipartResponse(
            [textContent, uiResource],
            {
              type,
              title,
              data,
              dataPoints: data.length,
            },
          );
        }

        case 'wait_for_user_resume': {
          const message = String(a.message || '');
          const resumeInstruction = String(a.resumeInstruction || '');

          if (!message) {
            return createMCPTextResponse('Message is required');
          }

          if (!resumeInstruction) {
            return createMCPTextResponse('Resume instruction is required');
          }

          // Generate startedAt timestamp
          const startedAt = new Date().toISOString();

          // Generate HTML
          const html = buildWaitHtml(message, resumeInstruction, startedAt);
          const uri = `ui://wait/${Date.now()}` as `ui://${string}`;
          const uiResource = createUiResourceFromHtml(html, uri);

          // Text content
          const textContent: MCPContent = {
            type: 'text',
            text: `⏳ Waiting: ${message}\nResume instruction: ${resumeInstruction}`,
          } as MCPContent;

          // Return multipart
          return createMCPStructuredMultipartResponse(
            [textContent, uiResource],
            { waiting: true, resumeInstruction, startedAt },
          );
        }

        case 'resume_from_wait': {
          const resumeInstruction = String(a.resumeInstruction || '');
          const startedAt = String(a.startedAt || '');

          if (!resumeInstruction) {
            return createMCPTextResponse('Resume instruction is required');
          }

          if (!startedAt) {
            return createMCPTextResponse('Started at timestamp is required');
          }

          // Calculate duration
          const startedAtDate = new Date(startedAt);
          const resumedAt = new Date();
          const duration = formatDuration(
            resumedAt.getTime() - startedAtDate.getTime(),
          );

          // Build agent-friendly text response
          const text = [
            `✅ User resumed after waiting ${duration}`,
            '',
            `Resume instruction: ${resumeInstruction}`,
            `Started: ${startedAt}`,
            `Ended: ${resumedAt.toISOString()}`,
          ].join('\n');

          return createMCPStructuredResponse(text, {
            resumed: true,
            duration,
            resumeInstruction,
            startedAt,
            resumedAt: resumedAt.toISOString(),
          });
        }

        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (err) {
      return createMCPTextResponse(
        `Error in ui-tools: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  },
};

export default uiTools;
