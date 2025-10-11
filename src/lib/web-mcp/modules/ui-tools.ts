/**
 * UI Tools - Built-in MCP server for user interaction
 *
 * Provides tools for creating interactive UI elements that communicate back to the agent:
 * - prompt_user: Display interactive prompts (text input, select, multiselect)
 * - reply_prompt: Receive user responses from prompt UI
 * - visualize_data: Create simple data visualizations (bar/line charts)
 */

import {
  createMCPStructuredResponse,
  createMCPTextResponse,
  createMCPStructuredMultipartResponse,
} from '@/lib/mcp-response-utils';
import type {
  MCPResponse,
  MCPTool,
  WebMCPServer,
  MCPContent,
  ServiceInfo,
} from '@/lib/mcp-types';
import { createUIResource, type UIResource } from '@mcp-ui/server';

// HTML escaping for XSS prevention
function escapeHtml(s: string): string {
  if (!s) return '';
  const map: { [char: string]: string } = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  };
  return String(s).replace(/[&<>"']/g, (c) => map[c]);
}

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

// Wait context type (structured information)
interface WaitContext {
  startedAt: string; // ISO 8601 timestamp (server auto-generated)
  reason: string; // Wait reason
  command: string; // Command/task being executed
  nextAction: string; // Action to perform after resume
}

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
 * Generate HTML for wait UI
 */
function buildWaitHtml(message: string, context: WaitContext): string {
  const escapedMessage = escapeHtml(message);
  // Serialize context for safe embedding in JS. Do NOT HTML-escape JSON (that breaks JS).
  // Escape closing </script> to avoid breaking out of the script block.
  const serializedContext = JSON.stringify(context).replace(
    /<\/(script)>/gi,
    '\\u003C/$1',
  );

  // Also produce an HTML-escaped JSON string for embedding in attributes so tests
  // and HTML inspection can find the escaped representation.
  const htmlEscapedContext = escapeHtml(JSON.stringify(context));

  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body {
      font-family: system-ui, -apple-system, sans-serif;
      margin: 0;
      padding: 16px;
      background: #f9fafb;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
    }
    .wait-container {
      max-width: 500px;
      background: white;
      border-radius: 8px;
      padding: 32px;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
      text-align: center;
    }
    .spinner {
      font-size: 48px;
      margin-bottom: 16px;
      animation: spin 2s linear infinite;
    }
    @keyframes spin {
      from { transform: rotate(0deg); }
      to { transform: rotate(360deg); }
    }
    @media (prefers-reduced-motion: reduce) {
      .spinner { animation: none; }
    }
    .message {
      font-size: 18px;
      font-weight: 600;
      color: #1f2937;
      margin-bottom: 24px;
    }
    .btn-continue {
      padding: 12px 24px;
      background: #3b82f6;
      color: white;
      border: none;
      border-radius: 6px;
      font-size: 16px;
      font-weight: 500;
      cursor: pointer;
      transition: background 0.2s;
    }
    .btn-continue:hover {
      background: #2563eb;
    }
    .btn-continue:focus {
      outline: none;
      box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.3);
    }
  </style>
</head>
<body>
  <div class="wait-container" role="dialog" aria-modal="true"
    aria-labelledby="wait-message" data-context="${htmlEscapedContext}">
    <div class="spinner" aria-hidden="true">⏳</div>
    <div id="wait-message" class="message">${escapedMessage}</div>
    <button class="btn-continue" onclick="handleContinue()" autofocus>
      계속
    </button>
  </div>
  <script>
    const context = ${serializedContext};

    function handleContinue() {
      window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'resume_from_wait',
          params: { context }
        }
      }, '*');
    }

    // Keyboard support
    document.addEventListener('keydown', function(e) {
      if (e.key === 'Enter') {
        handleContinue();
      }
    });
  </script>
</body>
</html>`;
}

/**
 * Create a UIResource with proper serviceInfo for routing
 */
function createUiResourceFromHtml(
  html: string,
  uri: `ui://${string}`,
): UIResource & { serviceInfo?: ServiceInfo } {
  const res = createUIResource({
    uri,
    content: { type: 'rawHtml', htmlString: html },
    encoding: 'text',
  }) as UIResource & { serviceInfo?: ServiceInfo };

  // Attach serviceInfo for proper routing
  res.serviceInfo = {
    serverName: 'ui',
    toolName: '',
    backendType: 'BuiltInWeb',
  };

  return res;
}

/**
 * Generate HTML for text input prompt
 */
function buildTextPromptHtml(messageId: string, prompt: string): string {
  const escapedPrompt = escapeHtml(prompt);
  const escapedMessageId = escapeHtml(messageId);

  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body {
      font-family: system-ui, -apple-system, sans-serif;
      margin: 0;
      padding: 16px;
      background: #f9fafb;
    }
    .prompt-container {
      max-width: 600px;
      background: white;
      border-radius: 8px;
      padding: 20px;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }
    .prompt-label {
      font-weight: 600;
      margin-bottom: 12px;
      color: #1f2937;
    }
    .prompt-input {
      width: 100%;
      padding: 10px 12px;
      border: 1px solid #d1d5db;
      border-radius: 6px;
      font-size: 14px;
      box-sizing: border-box;
    }
    .prompt-input:focus {
      outline: none;
      border-color: #3b82f6;
      box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
    }
    .prompt-actions {
      margin-top: 16px;
      display: flex;
      gap: 8px;
      justify-content: flex-end;
    }
    .btn {
      padding: 8px 16px;
      border-radius: 6px;
      border: none;
      font-size: 14px;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-primary {
      background: #3b82f6;
      color: white;
    }
    .btn-primary:hover {
      background: #2563eb;
    }
    .btn-secondary {
      background: #e5e7eb;
      color: #374151;
    }
    .btn-secondary:hover {
      background: #d1d5db;
    }
  </style>
</head>
<body>
  <div class="prompt-container">
    <div class="prompt-label">${escapedPrompt}</div>
    <input type="text" id="userInput" class="prompt-input" placeholder="Type your answer...">
    <div class="prompt-actions">
      <button class="btn btn-secondary" onclick="handleCancel()">Cancel</button>
      <button class="btn btn-primary" onclick="handleSubmit()">Submit</button>
    </div>
  </div>
  <script>
    const messageId = '${escapedMessageId}';
    const input = document.getElementById('userInput');

    input.addEventListener('keypress', function(e) {
      if (e.key === 'Enter') {
        handleSubmit();
      }
    });

    function handleSubmit() {
      const answer = input.value.trim();
      if (!answer) {
        alert('Please enter a response');
        return;
      }
      window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'reply_prompt',
          params: { messageId, answer }
        }
      }, '*');
    }

    function handleCancel() {
      window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'reply_prompt',
          params: { messageId, answer: null, cancelled: true }
        }
      }, '*');
    }

    // Auto-focus input
    input.focus();
  </script>
</body>
</html>`;
}

/**
 * Generate HTML for select/multiselect prompt
 */
function buildSelectPromptHtml(
  messageId: string,
  prompt: string,
  options: string[],
  multiselect: boolean,
): string {
  const escapedPrompt = escapeHtml(prompt);
  const escapedMessageId = escapeHtml(messageId);

  const optionsHtml = options
    .map((opt, idx) => {
      const escapedOpt = escapeHtml(opt);
      const inputType = multiselect ? 'checkbox' : 'radio';
      return `
      <label class="option-item">
        <input type="${inputType}" name="option" value="${idx}" class="option-input">
        <span class="option-label">${escapedOpt}</span>
      </label>`;
    })
    .join('');

  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body {
      font-family: system-ui, -apple-system, sans-serif;
      margin: 0;
      padding: 16px;
      background: #f9fafb;
    }
    .prompt-container {
      max-width: 600px;
      background: white;
      border-radius: 8px;
      padding: 20px;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }
    .prompt-label {
      font-weight: 600;
      margin-bottom: 16px;
      color: #1f2937;
    }
    .options-list {
      display: flex;
      flex-direction: column;
      gap: 12px;
      margin-bottom: 20px;
    }
    .option-item {
      display: flex;
      align-items: center;
      padding: 12px;
      border: 1px solid #e5e7eb;
      border-radius: 6px;
      cursor: pointer;
      transition: all 0.2s;
    }
    .option-item:hover {
      background: #f9fafb;
      border-color: #3b82f6;
    }
    .option-input {
      margin-right: 12px;
      cursor: pointer;
    }
    .option-label {
      flex: 1;
      font-size: 14px;
      color: #374151;
    }
    .prompt-actions {
      display: flex;
      gap: 8px;
      justify-content: flex-end;
    }
    .btn {
      padding: 8px 16px;
      border-radius: 6px;
      border: none;
      font-size: 14px;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-primary {
      background: #3b82f6;
      color: white;
    }
    .btn-primary:hover {
      background: #2563eb;
    }
    .btn-secondary {
      background: #e5e7eb;
      color: #374151;
    }
    .btn-secondary:hover {
      background: #d1d5db;
    }
  </style>
</head>
<body>
  <div class="prompt-container">
    <div class="prompt-label">${escapedPrompt}</div>
    <div class="options-list">
      ${optionsHtml}
    </div>
    <div class="prompt-actions">
      <button class="btn btn-secondary" onclick="handleCancel()">Cancel</button>
      <button class="btn btn-primary" onclick="handleSubmit()">Submit</button>
    </div>
  </div>
  <script>
    const messageId = '${escapedMessageId}';
    const isMultiselect = ${multiselect};
    const options = ${JSON.stringify(options)};

    function handleSubmit() {
      const selected = Array.from(document.querySelectorAll('input[name="option"]:checked'))
        .map(input => parseInt(input.value));

      if (selected.length === 0) {
        alert('Please select at least one option');
        return;
      }

      const answer = isMultiselect
        ? selected.map(idx => options[idx])
        : options[selected[0]];

      window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'reply_prompt',
          params: { messageId, answer }
        }
      }, '*');
    }

    function handleCancel() {
      window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'reply_prompt',
          params: { messageId, answer: null, cancelled: true }
        }
      }, '*');
    }
  </script>
</body>
</html>`;
}

/**
 * Generate HTML for data visualization
 */
function buildVisualizationHtml(
  type: 'bar' | 'line',
  data: Array<{ label: string; value: number }>,
): string {
  const maxValue = Math.max(...data.map((d) => d.value), 1);
  const chartHeight = 300;
  const barWidth = Math.max(40, Math.min(80, 400 / data.length));

  if (type === 'bar') {
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

    return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body { margin: 0; padding: 16px; background: #f9fafb; }
    svg { max-width: 100%; height: auto; }
  </style>
</head>
<body>
  <svg viewBox="0 0 ${data.length * (barWidth + 10) + 100} ${chartHeight + 20}"
       xmlns="http://www.w3.org/2000/svg">
    ${bars}
  </svg>
</body>
</html>`;
  } else {
    // Line chart
    const points = data.map((item, idx) => {
      const x = (idx / (data.length - 1)) * 500 + 50;
      const y = chartHeight - (item.value / maxValue) * (chartHeight - 40) - 20;
      return `${x},${y}`;
    });

    const labels = data
      .map((item, idx) => {
        const x = (idx / (data.length - 1)) * 500 + 50;
        const escapedLabel = escapeHtml(item.label);
        return `
        <text x="${x}" y="${chartHeight - 5}"
              text-anchor="middle" font-size="12" fill="#6b7280">
          ${escapedLabel}
        </text>
        <circle cx="${x}" cy="${chartHeight - (item.value / maxValue) * (chartHeight - 40) - 20}"
                r="4" fill="#3b82f6" />
        <text x="${x}" y="${chartHeight - (item.value / maxValue) * (chartHeight - 40) - 30}"
              text-anchor="middle" font-size="12" font-weight="600" fill="#1f2937">
          ${item.value}
        </text>`;
      })
      .join('');

    return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body { margin: 0; padding: 16px; background: #f9fafb; }
    svg { max-width: 100%; height: auto; }
  </style>
</head>
<body>
  <svg viewBox="0 0 600 ${chartHeight + 20}" xmlns="http://www.w3.org/2000/svg">
    <polyline points="${points.join(' ')}"
              fill="none" stroke="#3b82f6" stroke-width="2" />
    ${labels}
  </svg>
</body>
</html>`;
  }
}

const tools: MCPTool[] = [
  {
    name: 'prompt_user',
    description:
      'Display an interactive prompt to the user (text input, select, or multiselect)',
    inputSchema: {
      type: 'object',
      properties: {
        prompt: {
          type: 'string',
          description: 'The question or instruction to show the user',
        },
        type: {
          type: 'string',
          enum: ['text', 'select', 'multiselect'],
          description: 'Type of prompt UI to display',
        },
        options: {
          type: 'array',
          items: { type: 'string' },
          description:
            'Options for select/multiselect (required for those types)',
        },
      },
      required: ['prompt', 'type'],
    },
  },
  {
    name: 'reply_prompt',
    description:
      'Receive user response from prompt UI (automatically called by UI action)',
    inputSchema: {
      type: 'object',
      properties: {
        messageId: {
          type: 'string',
          description: 'ID of the prompt being replied to',
        },
        answer: {
          description:
            'User answer (string, array of strings, or null if cancelled)',
        },
        cancelled: {
          type: 'boolean',
          description: 'Whether the user cancelled the prompt',
        },
      },
      required: ['messageId'],
    },
  },
  {
    name: 'visualize_data',
    description: 'Create a simple data visualization (bar or line chart)',
    inputSchema: {
      type: 'object',
      properties: {
        type: {
          type: 'string',
          enum: ['bar', 'line'],
          description: 'Type of chart to create',
        },
        data: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              label: {
                type: 'string',
                description: 'Label for this data point',
              },
              value: { type: 'number', description: 'Numeric value' },
            },
            required: ['label', 'value'],
          },
          description: 'Data points to visualize',
        },
        title: {
          type: 'string',
          description: 'Optional title for the chart',
        },
      },
      required: ['type', 'data'],
    },
  },
  {
    name: 'wait_for_user_resume',
    description:
      'Display wait UI with continue button for long operations, especially useful when repetitive polling is needed',
    inputSchema: {
      type: 'object',
      properties: {
        message: {
          type: 'string',
          description: 'Message to display to user',
        },
        context: {
          type: 'object',
          properties: {
            reason: {
              type: 'string',
              description: 'Why waiting (for agent context)',
            },
            command: {
              type: 'string',
              description: 'Command or task being executed',
            },
            nextAction: {
              type: 'string',
              description: 'What to do after resume',
            },
          },
          required: ['reason', 'command', 'nextAction'],
        },
      },
      required: ['message', 'context'],
    },
  },
  {
    name: 'resume_from_wait',
    description: 'Resume from wait (called by UI button click)',
    inputSchema: {
      type: 'object',
      properties: {
        context: {
          type: 'object',
          properties: {
            startedAt: { type: 'string' },
            reason: { type: 'string' },
            command: { type: 'string' },
            nextAction: { type: 'string' },
          },
          required: ['startedAt', 'reason', 'command', 'nextAction'],
        },
        sessionId: {
          type: 'string',
          description: 'Optional session ID for validation',
        },
      },
      required: ['context'],
    },
  },
];

const uiTools: WebMCPServer = {
  name: 'ui',
  version: '0.1.0',
  description:
    'Built-in UI interaction tools for user prompts and visualizations',
  tools,

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
          const html = buildVisualizationHtml(type, data);

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
          const contextInput = a.context as Omit<WaitContext, 'startedAt'>;

          if (!message) {
            return createMCPTextResponse('Message is required');
          }

          if (
            !contextInput ||
            !contextInput.reason ||
            !contextInput.command ||
            !contextInput.nextAction
          ) {
            return createMCPTextResponse(
              'Context with reason, command, nextAction required',
            );
          }

          // Add startedAt timestamp
          const context: WaitContext = {
            ...contextInput,
            startedAt: new Date().toISOString(),
          };

          // Generate HTML
          const html = buildWaitHtml(message, context);
          const uri = `ui://wait/${Date.now()}` as `ui://${string}`;
          const uiResource = createUiResourceFromHtml(html, uri);

          // Text content
          const textContent: MCPContent = {
            type: 'text',
            text: `⏳ Waiting: ${message}\nReason: ${context.reason}`,
          } as MCPContent;

          // Return multipart
          return createMCPStructuredMultipartResponse(
            [textContent, uiResource],
            { waiting: true, context },
          );
        }

        case 'resume_from_wait': {
          const context = a.context as WaitContext;

          if (!context || !context.startedAt) {
            return createMCPTextResponse('Valid context required');
          }

          // Calculate duration
          const startedAt = new Date(context.startedAt);
          const resumedAt = new Date();
          const duration = formatDuration(
            resumedAt.getTime() - startedAt.getTime(),
          );

          // Build agent-friendly text response
          const text = [
            `✅ User resumed after waiting ${duration}`,
            '',
            `What was waiting: ${context.reason}`,
            `Command/Task: ${context.command}`,
            `Started: ${context.startedAt}`,
            `Ended: ${resumedAt.toISOString()}`,
            '',
            `Next action: ${context.nextAction}`,
          ].join('\n');

          return createMCPStructuredResponse(text, {
            resumed: true,
            duration,
            ...context,
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
