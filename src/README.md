# Client Example (javascript)

## Groq

### list models

```js
import Groq from 'groq-sdk';

const groq = new Groq({ apiKey: process.env.GROQ_API_KEY });

const getModels = async () => {
  return await groq.models.list();
};

getModels().then((models) => {
  // console.log(models);
});
```

### Reasoning

```js
import { Groq } from 'groq-sdk';

const groq = new Groq();

const chatCompletion = await groq.chat.completions.create({
  messages: [
    {
      role: 'user',
      content: '',
    },
  ],
  model: 'deepseek-r1-distill-llama-70b',
  temperature: 0.6,
  max_completion_tokens: 4096,
  top_p: 0.95,
  stream: true,
  // reasoning_effort is specific to O1-like models, check SDK support
  stop: null,
});

for await (const chunk of chatCompletion) {
  process.stdout.write(chunk.choices[0]?.delta?.content || '');
}
```

### Tool Use

```js
import Groq from 'groq-sdk';

const client = new Groq();
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
    const availableFunctions = {
      calculate: (args) => {
        // Warning: eval is dangerous in production
        return eval(args.expression);
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

```js
import { OpenAI } from 'openai';

const openai = new OpenAI();

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
  store: true,
});

console.log(completion.choices[0].message.tool_calls);
```

### Reasoning

```js
import OpenAI from 'openai';

const openai = new OpenAI();

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

```js
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic();

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
