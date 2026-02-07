/**
 * Comprehensive test suite for ollama-core pure functions
 * Tests all core logic patterns used by use-ai-service.ts hook
 *
 * Usage:
 *   pnpm tsx scripts/test_ollama_core.ts
 *
 * Requirements:
 *   - Ollama server running on http://127.0.0.1:11434
 *   - At least one model installed (e.g., qwen3:8b, llama3.1:8b)
 *
 * Tests:
 *   - Message conversion (user, assistant, tool)
 *   - Tool schema conversion (MCP → Ollama format)
 *   - Model capability detection
 *   - Reasoning parameter determination
 *   - Chunk processing (content, thinking, tool_calls)
 *   - Advanced scenarios (multi-turn, incremental tool calls)
 *   - Edge cases (malformed JSON, empty arrays, unicode)
 *   - Live Ollama API integration
 */

import { Ollama } from 'ollama/browser';
import {
  convertMCPToolsToOllamaTools,
  convertToOllamaMessages,
  processChunk,
  getModelToolSupport,
  determineThinkParam,
  consoleLogger,
  noopLogger,
} from '../src/lib/ai-service/ollama-core';
import type { Message } from '../src/models/chat';
import type { MCPTool } from '../src/lib/mcp';

const DEFAULT_HOST = 'http://127.0.0.1:11434';

// Test data
const testMessages: Message[] = [
  {
    id: '1',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'user',
    content: [{ type: 'text', text: 'Hello, how are you?' }],
  },
  {
    id: '2',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'assistant',
    content: [{ type: 'text', text: 'I am doing well, thank you!' }],
  },
  {
    id: '3',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'user',
    content: [{ type: 'text', text: 'What can you help me with?' }],
  },
];

const testMessagesWithTools: Message[] = [
  {
    id: '1',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'user',
    content: [{ type: 'text', text: 'What is the weather in Seoul?' }],
  },
  {
    id: '2',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'assistant',
    content: [{ type: 'text', text: 'Let me check the weather for you.' }],
    tool_calls: [
      {
        id: 'call_123',
        type: 'function',
        function: {
          name: 'get_weather',
          arguments: JSON.stringify({ location: 'Seoul' }),
        },
      },
    ],
  },
  {
    id: '3',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'tool',
    content: [
      { type: 'text', text: 'Temperature: 15°C, Conditions: Partly cloudy' },
    ],
    tool_call_id: 'call_123',
  },
  {
    id: '4',
    sessionId: 'test-session',
    threadId: 'test-thread',
    role: 'assistant',
    content: [
      {
        type: 'text',
        text: 'The weather in Seoul is 15°C and partly cloudy.',
      },
    ],
  },
];

const testTools: MCPTool[] = [
  {
    name: 'get_weather',
    description: 'Get the current weather for a location',
    inputSchema: {
      type: 'object',
      properties: {
        location: {
          type: 'string',
          description: 'The city and state, e.g. San Francisco, CA',
        },
      },
      required: ['location'],
    },
  },
  {
    name: 'calculator',
    description: 'Perform basic arithmetic operations',
    inputSchema: {
      type: 'object',
      properties: {
        operation: {
          type: 'string',
          enum: ['add', 'subtract', 'multiply', 'divide'],
        },
        a: { type: 'number' },
        b: { type: 'number' },
      },
      required: ['operation', 'a', 'b'],
    },
  },
];

async function testConversions() {
  console.log('\n=== Testing Message Conversions ===\n');

  // Test basic message conversion
  const ollamaMessages = convertToOllamaMessages(
    testMessages,
    'You are a helpful assistant.',
    consoleLogger,
  );
  console.log('Basic messages converted:');
  console.log(JSON.stringify(ollamaMessages, null, 2));

  // Test message with tools
  const ollamaMessagesWithTools = convertToOllamaMessages(
    testMessagesWithTools,
    undefined,
    consoleLogger,
  );
  console.log('\nMessages with tools converted:');
  console.log(JSON.stringify(ollamaMessagesWithTools, null, 2));
}

function testToolConversion() {
  console.log('\n=== Testing Tool Conversion ===\n');

  const ollamaTools = convertMCPToolsToOllamaTools(testTools, consoleLogger);
  console.log('MCP Tools converted to Ollama format:');
  console.log(JSON.stringify(ollamaTools, null, 2));
}

function testModelCapabilities() {
  console.log('\n=== Testing Model Capabilities ===\n');

  const models = [
    'llama3.1:8b',
    'llama3.2:3b',
    'qwen2.5:14b',
    'mistral:7b',
    'dolphin-mixtral:8x7b',
    'gemma:2b',
    'phi3:mini',
  ];

  models.forEach((model) => {
    const supportsTools = getModelToolSupport(model);
    console.log(`${model}: ${supportsTools ? '✓' : '✗'} supports tools`);
  });
}

function testReasoningParam() {
  console.log('\n=== Testing Reasoning Parameter Determination ===\n');

  const testCases = [
    {
      enableReasoning: false,
      reasoningEffort: undefined,
      modelSupportsThinking: true,
    },
    {
      enableReasoning: true,
      reasoningEffort: undefined,
      modelSupportsThinking: true,
    },
    {
      enableReasoning: true,
      reasoningEffort: 'medium' as const,
      modelSupportsThinking: true,
    },
    {
      enableReasoning: true,
      reasoningEffort: 'high' as const,
      modelSupportsThinking: false,
    },
  ];

  testCases.forEach((testCase, index) => {
    const result = determineThinkParam(
      testCase.enableReasoning,
      testCase.reasoningEffort,
      testCase.modelSupportsThinking,
      consoleLogger,
    );
    console.log(
      `Case ${index + 1}: enableReasoning=${testCase.enableReasoning}, ` +
        `reasoningEffort=${testCase.reasoningEffort}, ` +
        `modelSupportsThinking=${testCase.modelSupportsThinking} => ${result}`,
    );
  });
}

function testChunkProcessing() {
  console.log('\n=== Testing Chunk Processing ===\n');

  // Test normal content chunk
  const contentChunk = {
    message: {
      content: 'Hello, this is a test response.',
    },
  };
  const contentResult = processChunk(contentChunk, consoleLogger);
  console.log('Content chunk result:', contentResult);

  // Test thinking chunk (reasoning mode)
  const thinkingChunk = {
    message: {
      thinking: 'Let me analyze this question carefully...',
    },
  };
  const thinkingResult = processChunk(thinkingChunk, consoleLogger);
  console.log('Thinking chunk result:', thinkingResult);

  // Test thinking in content field with <think> tags (Ollama format)
  const thinkingInContentChunk = {
    message: {
      content:
        '<think>\nLet me break this down step by step...\n</think>\n\nThe answer is 42.',
    },
  };
  const thinkingInContentResult = processChunk(
    thinkingInContentChunk,
    consoleLogger,
  );
  console.log('Thinking in content result:', thinkingInContentResult);
  const parsed = thinkingInContentResult
    ? JSON.parse(thinkingInContentResult)
    : null;
  if (parsed) {
    console.log('  - Thinking extracted:', parsed.thinking);
    console.log('  - Content extracted:', parsed.content);
    console.log(
      '  - Tags removed:',
      !parsed.thinking?.includes('<think>') &&
        !parsed.content?.includes('<think>'),
    );
  }

  // Test tool call chunk
  const toolCallChunk = {
    message: {
      tool_calls: [
        {
          id: 'call_456',
          type: 'function',
          function: {
            name: 'get_weather',
            arguments: { location: 'Tokyo' },
          },
        },
      ],
    },
  };
  const toolCallResult = processChunk(toolCallChunk, consoleLogger);
  console.log('Tool call chunk result:', toolCallResult);

  // Test combined chunk
  const combinedChunk = {
    message: {
      content: 'Based on the data, ',
      thinking: 'I need to calculate this...',
      tool_calls: [
        {
          type: 'function',
          function: {
            name: 'calculator',
            arguments: { operation: 'add', a: 5, b: 3 },
          },
        },
      ],
    },
  };
  const combinedResult = processChunk(combinedChunk, consoleLogger);
  console.log('Combined chunk result:', combinedResult);

  // Test invalid chunk
  const invalidChunk = { invalid: 'data' };
  const invalidResult = processChunk(invalidChunk, consoleLogger);
  console.log('Invalid chunk result:', invalidResult);
}

async function testActualOllamaCall() {
  console.log('\n=== Testing Actual Ollama API Call ===\n');

  try {
    const ollama = new Ollama({ host: 'http://localhost:11434' });

    // List available models
    const models = await ollama.list();
    console.log('Available models:');
    models.models.forEach((model) => {
      console.log(`  - ${model.name}`);
    });

    if (models.models.length === 0) {
      console.log('No models found. Skipping API test.');
      return;
    }

    // Use the first available model
    const modelName = models.models[0].name;
    console.log(`\nUsing model: ${modelName}`);

    // Convert messages using core function
    const ollamaMessages = convertToOllamaMessages(
      [
        {
          id: '1',
          sessionId: 'test-session',
          threadId: 'test-thread',
          role: 'user',
          content: [{ type: 'text', text: 'Say hello in one short sentence.' }],
        },
      ],
      undefined,
      consoleLogger,
    );

    console.log('\nSending request to Ollama...');
    const response = await ollama.chat({
      model: modelName,
      messages: ollamaMessages as Array<{
        role: string;
        content: string;
      }>,
      stream: false,
    });

    console.log('\nResponse from Ollama:');
    console.log(response.message.content);

    // Test streaming with chunk processing
    console.log('\n--- Testing Streaming with Chunk Processing ---');
    const streamResponse = await ollama.chat({
      model: modelName,
      messages: ollamaMessages as Array<{
        role: string;
        content: string;
      }>,
      stream: true,
    });

    console.log('\nStreaming response:');
    for await (const chunk of streamResponse) {
      const processed = processChunk(chunk, consoleLogger);
      if (processed) {
        const data = JSON.parse(processed);
        if (data.content) {
          process.stdout.write(data.content);
        }
      }
    }
    console.log('\n');
  } catch (error) {
    console.error('Ollama API test failed:', error);
    console.log(
      '\nNote: Make sure Ollama is running on http://localhost:11434',
    );
  }
}

async function main() {
  console.log('=================================================');
  console.log('Ollama Core Functions - Comprehensive Test Suite');
  console.log('=================================================');

  try {
    await testConversions();
    testToolConversion();
    testModelCapabilities();
    testReasoningParam();
    testChunkProcessing();
    await testAdvancedScenarios();
    await testEdgeCases();
    await testActualOllamaCall();

    console.log('\n=== All Tests Completed Successfully ===\n');
  } catch (error) {
    console.error('\n!!! Test Failed !!!');
    console.error(error);
    process.exit(1);
  }
}

/**
 * Test advanced scenarios from use-ai-service.ts
 */
async function testAdvancedScenarios() {
  console.log('\n=== Testing Advanced Scenarios ===\n');

  // Test 1: Multi-turn conversation with tool calls
  console.log('--- Scenario 1: Multi-turn with Tool Calls ---');
  const multiTurnMessages: Message[] = [
    {
      id: '1',
      sessionId: 'test',
      threadId: 'test',
      role: 'user',
      content: [{ type: 'text', text: 'What is the weather in Tokyo?' }],
    },
    {
      id: '2',
      sessionId: 'test',
      threadId: 'test',
      role: 'assistant',
      content: [{ type: 'text', text: 'Let me check that for you.' }],
      tool_calls: [
        {
          id: 'call_1',
          type: 'function',
          function: {
            name: 'get_weather',
            arguments: JSON.stringify({ location: 'Tokyo', units: 'celsius' }),
          },
        },
      ],
    },
    {
      id: '3',
      sessionId: 'test',
      threadId: 'test',
      role: 'tool',
      content: [{ type: 'text', text: '{"temp": 18, "condition": "sunny"}' }],
      tool_call_id: 'call_1',
    },
    {
      id: '4',
      sessionId: 'test',
      threadId: 'test',
      role: 'assistant',
      content: [
        {
          type: 'text',
          text: 'The weather in Tokyo is 18°C and sunny.',
        },
      ],
    },
    {
      id: '5',
      sessionId: 'test',
      threadId: 'test',
      role: 'user',
      content: [{ type: 'text', text: 'How about Seoul?' }],
    },
  ];

  const converted = convertToOllamaMessages(
    multiTurnMessages,
    'You are a weather assistant.',
    consoleLogger,
  );
  console.log(
    `✅ Converted ${multiTurnMessages.length} messages to ${converted.length} Ollama messages`,
  );
  console.log('Sample structure:');
  console.log(JSON.stringify(converted[0], null, 2));
  console.log(`... (${converted.length - 1} more)`);

  // Test 2: Incremental tool call chunks (OpenAI pattern)
  console.log('\n--- Scenario 2: Incremental Tool Call Assembly ---');
  const chunks = [
    {
      message: {
        tool_calls: [
          {
            index: 0,
            id: 'call_abc',
            type: 'function',
            function: { name: 'search', arguments: '{"q' },
          },
        ],
      },
    },
    {
      message: {
        tool_calls: [{ index: 0, function: { arguments: 'uery":"' } }],
      },
    },
    {
      message: {
        tool_calls: [{ index: 0, function: { arguments: 'test"}' } }],
      },
    },
  ];

  console.log('Processing incremental chunks (simulating OpenAI pattern):');
  chunks.forEach((chunk, i) => {
    const result = processChunk(chunk, consoleLogger);
    console.log(`  Chunk ${i + 1}: ${result ? 'parsed' : 'null'}`);
  });

  // Test 3: Complete tool call (Ollama/Gemini pattern)
  console.log('\n--- Scenario 3: Complete Tool Call (Ollama Pattern) ---');
  const completeToolChunk = {
    message: {
      tool_calls: [
        {
          id: 'call_xyz',
          type: 'function',
          function: {
            name: 'calculator',
            arguments: { operation: 'multiply', a: 7, b: 6 },
          },
        },
      ],
    },
  };

  const completeResult = processChunk(completeToolChunk, consoleLogger);
  console.log('Complete tool call result:', completeResult);
  if (completeResult) {
    const parsed = JSON.parse(completeResult);
    console.log('✅ Tool calls:', parsed.tool_calls?.length || 0);
    console.log(
      '   Arguments type:',
      typeof parsed.tool_calls?.[0]?.function?.arguments,
    );
  }

  // Test 4: Reasoning mode chunks with thinking
  console.log('\n--- Scenario 4: Reasoning Mode with Thinking ---');
  const reasoningChunks = [
    { message: { thinking: 'Let me analyze this problem...' } },
    { message: { thinking: ' I need to break it down...' } },
    { message: { content: 'The answer is ' } },
    { message: { content: '42.' } },
  ];

  let accumulatedThinking = '';
  let accumulatedContent = '';

  reasoningChunks.forEach((chunk) => {
    const result = processChunk(chunk, noopLogger); // Use noop for cleaner output
    if (result) {
      const parsed = JSON.parse(result);
      if (parsed.thinking) accumulatedThinking += parsed.thinking;
      if (parsed.content) accumulatedContent += parsed.content;
    }
  });

  console.log(`Accumulated thinking: "${accumulatedThinking}"`);
  console.log(`Accumulated content: "${accumulatedContent}"`);

  // Test 5: Mixed content + thinking + tool_calls
  console.log('\n--- Scenario 5: Combined Chunk (All Fields) ---');
  const mixedChunk = {
    message: {
      content: 'Based on my analysis, ',
      thinking: 'I should use the calculator tool here...',
      tool_calls: [
        {
          id: 'call_mixed',
          type: 'function',
          function: {
            name: 'calculator',
            arguments: { operation: 'add', a: 10, b: 20 },
          },
        },
      ],
    },
  };

  const mixedResult = processChunk(mixedChunk, consoleLogger);
  console.log('Mixed chunk result:', mixedResult);
}

/**
 * Test edge cases and error handling
 */
async function testEdgeCases() {
  console.log('\n=== Testing Edge Cases ===\n');

  // Test 1: Malformed JSON in tool arguments
  console.log('--- Edge Case 1: Malformed Tool Arguments ---');
  const malformedMessage: Message = {
    id: '1',
    sessionId: 'test',
    threadId: 'test',
    role: 'assistant',
    content: [{ type: 'text', text: 'Using tool...' }],
    tool_calls: [
      {
        id: 'call_bad',
        type: 'function',
        function: {
          name: 'bad_tool',
          arguments: '{broken json',
        },
      },
    ],
  };

  try {
    const converted = convertToOllamaMessages(
      [malformedMessage],
      undefined,
      consoleLogger,
    );
    console.log('✅ Handled malformed JSON gracefully');
    console.log(
      '   Tool call args:',
      converted[0].tool_calls?.[0]?.function?.arguments,
    );
  } catch {
    console.log('❌ Failed to handle malformed JSON');
  }

  // Test 2: Empty messages array
  console.log('\n--- Edge Case 2: Empty Messages Array ---');
  try {
    convertToOllamaMessages([], undefined, consoleLogger);
    console.log('❌ Should have thrown error for empty array');
  } catch (error) {
    console.log('✅ Correctly rejected empty messages array');
  }

  // Test 3: Missing role field
  console.log('\n--- Edge Case 3: Invalid Message Structure ---');
  const invalidChunk = {
    message: {
      role: undefined,
      content: 'test',
    },
  };

  const invalidResult = processChunk(invalidChunk, consoleLogger);
  console.log(
    'Invalid chunk handled:',
    invalidResult !== null ? 'processed' : 'rejected',
  );

  // Test 4: Very long system prompt
  console.log('\n--- Edge Case 4: Long System Prompt ---');
  const longPrompt = 'A'.repeat(5000);
  const simpleMessages: Message[] = [
    {
      id: '1',
      sessionId: 'test',
      threadId: 'test',
      role: 'user',
      content: [{ type: 'text', text: 'Hi' }],
    },
  ];

  const longPromptResult = convertToOllamaMessages(
    simpleMessages,
    longPrompt,
    noopLogger,
  );
  console.log(
    `✅ System prompt length: ${longPromptResult[0].content.length} chars`,
  );

  // Test 5: Unicode and special characters
  console.log('\n--- Edge Case 5: Unicode and Special Characters ---');
  const unicodeMessages: Message[] = [
    {
      id: '1',
      sessionId: 'test',
      threadId: 'test',
      role: 'user',
      content: [
        {
          type: 'text',
          text: '안녕하세요 🌍 Hello "quotes" & <tags>',
        },
      ],
    },
  ];

  const unicodeResult = convertToOllamaMessages(
    unicodeMessages,
    undefined,
    noopLogger,
  );
  console.log('✅ Unicode content:', unicodeResult[0].content);

  // Test 6: Null/undefined in chunk
  console.log('\n--- Edge Case 6: Null/Undefined Chunks ---');
  const edgeCases = [
    null,
    undefined,
    {},
    { message: null },
    { message: {} },
    { message: { content: null } },
    { message: { content: '' } },
  ];

  edgeCases.forEach((testCase, i) => {
    const result = processChunk(testCase, noopLogger);
    console.log(
      `  Case ${i + 1}: ${result === null ? 'null (expected)' : 'unexpected result'}`,
    );
  });

  // Test 7: Tool with no required fields
  console.log('\n--- Edge Case 7: Tool Schema Without Required Fields ---');
  const optionalTool: MCPTool = {
    name: 'optional_tool',
    description: 'All parameters are optional',
    inputSchema: {
      type: 'object',
      properties: {
        param1: { type: 'string' },
        param2: { type: 'number' },
      },
      // No required array
    },
  };

  const toolResult = convertMCPToolsToOllamaTools(
    [optionalTool],
    consoleLogger,
  );
  console.log(
    'Tool schema:',
    JSON.stringify(toolResult[0].function.parameters, null, 2),
  );

  // Test 8: Multiple tool calls in single message
  console.log('\n--- Edge Case 8: Multiple Tool Calls ---');
  const multiToolMessage: Message = {
    id: '1',
    sessionId: 'test',
    threadId: 'test',
    role: 'assistant',
    content: [{ type: 'text', text: 'Let me help with both.' }],
    tool_calls: [
      {
        id: 'call_1',
        type: 'function',
        function: {
          name: 'tool_a',
          arguments: JSON.stringify({ a: 1 }),
        },
      },
      {
        id: 'call_2',
        type: 'function',
        function: {
          name: 'tool_b',
          arguments: JSON.stringify({ b: 2 }),
        },
      },
      {
        id: 'call_3',
        type: 'function',
        function: {
          name: 'tool_c',
          arguments: JSON.stringify({ c: 3 }),
        },
      },
    ],
  };

  const multiToolResult = convertToOllamaMessages(
    [multiToolMessage],
    undefined,
    consoleLogger,
  );
  console.log(`✅ Converted ${multiToolMessage.tool_calls?.length} tool calls`);
  console.log(
    `   Result has ${multiToolResult[0].tool_calls?.length} tool calls`,
  );
}

main();
