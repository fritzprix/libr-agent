# Frontend AI Integration Examples

This document provides examples of how to integrate supported AI providers using their JavaScript SDKs within the LibrAgent frontend (React/Vite).

> **Note:** In the LibrAgent codebase, direct SDK usage is generally handled via the `AIServiceFactory` and specific service implementations in `src/lib/ai-service/`. These examples are for reference or prototyping purposes.
>
> **Important:** The main LibrAgent application manages API keys via the Settings Modal (stored in app configuration). The examples below use `import.meta.env` for simplicity in standalone scripts/prototypes, which may require a local `.env` file during development.

## Groq

### List Models

```javascript
import Groq from 'groq-sdk';

// Use import.meta.env for Vite environment variables
const groq = new Groq({
  apiKey: import.meta.env.VITE_GROQ_API_KEY,
  dangerouslyAllowBrowser: true, // Required for client-side usage
});

const getModels = async () => {
  return await groq.models.list();
};

getModels().then((models) => {
  console.log(models);
});
```

### Reasoning

```javascript
import { Groq } from 'groq-sdk';

const groq = new Groq({
  apiKey: import.meta.env.VITE_GROQ_API_KEY,
  dangerouslyAllowBrowser: true,
});

const chatCompletion = await groq.chat.completions.create({
  messages: [
    {
      role: 'user',
      content: 'Explain quantum entanglement',
    },
  ],
  model: 'deepseek-r1-distill-llama-70b',
  temperature: 0.6,
  max_completion_tokens: 4096,
  top_p: 0.95,
  stream: true,
  stop: null,
});

for await (const chunk of chatCompletion) {
  // Use console.log for output in browser console
  console.log(chunk.choices[0]?.delta?.content || '');
}
```

### Tool Use

```javascript
import Groq from 'groq-sdk';
import { Parser } from 'expr-eval';

const client = new Groq({
  apiKey: import.meta.env.VITE_GROQ_API_KEY,
  dangerouslyAllowBrowser: true,
});
const MODEL = 'llama-3.3-70b-versatile';

async function runConversation(userPrompt) {
  // Initialize the conversation with system and user messages
  const messages = [
    {
      role: 'system',
      content:
        'You are a calculator assistant. Use the calculate function to perform mathematical operations and provide the results.',
    },
    {
      role: 'user',
      content: userPrompt,
    },
  ];

  // Define the available tools
  const tools = [
    {
      type: 'function',
      function: {
        name: 'calculate',
        description: 'Evaluate a mathematical expression',
        parameters: {
          type: 'object',
          properties: {
            expression: {
              type: 'string',
              description: 'The mathematical expression to evaluate',
            },
          },
          required: ['expression'],
        },
      },
    },
  ];

  // Make the initial API call
  const response = await client.chat.completions.create({
    model: MODEL,
    messages: messages,
    stream: false,
    tools: tools,
    tool_choice: 'auto',
    max_tokens: 4096,
  });

  const responseMessage = response.choices[0].message;
  const toolCalls = responseMessage.tool_calls;

  if (toolCalls) {
    const parser = new Parser();

    const availableFunctions = {
      calculate: (args) => {
        // Avoid eval in production. Use a safe expression parser instead.
        try {
          return parser.evaluate(args.expression);
        } catch (e) {
          return `Error: ${e instanceof Error ? e.message : 'invalid expression'}`;
        }
      },
    };

    messages.push(responseMessage);

    for (const toolCall of toolCalls) {
      const functionName = toolCall.function.name;
      const functionArgs = JSON.parse(toolCall.function.arguments);
      const functionToCall = availableFunctions[functionName];
      const functionResponse = functionToCall(functionArgs);

      messages.push({
        tool_call_id: toolCall.id,
        role: 'tool',
        name: functionName,
        content: String(functionResponse),
      });
    }

    const secondResponse = await client.chat.completions.create({
      model: MODEL,
      messages: messages,
    });

    return secondResponse.choices[0].message.content;
  }
}

// Example usage
runConversation('What is 25 * 4 + 10?').then(console.log);
```

## OpenAI

### Tool Use

```javascript
import { OpenAI } from 'openai';

const openai = new OpenAI({
  apiKey: import.meta.env.VITE_OPENAI_API_KEY,
  dangerouslyAllowBrowser: true,
});

const tools = [
  {
    type: 'function',
    function: {
      name: 'get_weather',
      description: 'Get current temperature for a given location.',
      parameters: {
        type: 'object',
        properties: {
          location: {
            type: 'string',
            description: 'City and country e.g. Bogotá, Colombia',
          },
        },
        required: ['location'],
        additionalProperties: false,
      },
      strict: true,
    },
  },
];

const completion = await openai.chat.completions.create({
  model: 'gpt-4o',
  messages: [
    { role: 'user', content: 'What is the weather like in Paris today?' },
  ],
  tools,
  store: true, // Note: 'store' might be an OpenAI-specific feature requiring configured storage
});

console.log(completion.choices[0].message.tool_calls);
```

### Reasoning

```javascript
import OpenAI from 'openai';

const openai = new OpenAI({
  apiKey: import.meta.env.VITE_OPENAI_API_KEY,
  dangerouslyAllowBrowser: true,
});

const prompt = `
Write a bash script that takes a matrix represented as a string with 
format '[1,2],[3,4],[5,6]' and prints the transpose in the same format.
`;

const completion = await openai.chat.completions.create({
  model: 'o1-mini',
  reasoning_effort: 'medium',
  messages: [
    {
      role: 'user',
      content: prompt,
    },
  ],
  store: true,
});

console.log(completion.choices[0].message.content);
```

## Anthropic

### Streaming

```javascript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: import.meta.env.VITE_ANTHROPIC_API_KEY,
  dangerouslyAllowBrowser: true, // Required for client-side usage (matches LibrAgent Anthropic service)
});

const stream = await client.messages.create({
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello, Claude' }],
  model: 'claude-3-5-sonnet-20241022',
  stream: true,
});
for await (const messageStreamEvent of stream) {
  console.log(messageStreamEvent.type);
}
```

---

## 📦 Getting Started & Desktop App Downloads

To run these integration examples, you need the desktop application running locally. Download the latest installer for your platform:

<!-- RELEASE_DOWNLOADS_START -->

- **Windows:** [`LibrAgent_0.7.31_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent_0.7.31_x64-setup.exe) · [`LibrAgent_0.7.31_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent_0.7.31_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.7.31_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent_0.7.31_aarch64.dmg)
- **Linux:** [`LibrAgent_0.7.31_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent_0.7.31_amd64.AppImage) · [`LibrAgent_0.7.31_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent_0.7.31_amd64.deb) · [`LibrAgent-0.7.31-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.31/LibrAgent-0.7.31-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.7.31)
<!-- RELEASE_DOWNLOADS_END -->
