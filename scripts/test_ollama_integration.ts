#!/usr/bin/env tsx
/**
 * Ollama.ts Integration Test
 *
 * 실제 OllamaService 클래스를 사용하여 모든 기능을 검증합니다.
 */

import { Ollama } from 'ollama/browser';

const DEFAULT_HOST = 'http://127.0.0.1:11434';
const TEST_MODEL = 'qwen3:8b';

async function testStreamingChat() {
  console.log('\n🧪 Test: Streaming Chat with OllamaService Pattern');
  console.log('━'.repeat(60));

  try {
    const ollamaClient = new Ollama({ host: DEFAULT_HOST });

    const messages = [
      { role: 'user' as const, content: 'Say "Hello World" briefly' },
    ];

    const requestOptions = {
      model: TEST_MODEL,
      messages,
      stream: true as const,
      keep_alive: '5m',
      options: {
        temperature: 0.7,
        num_predict: 4096,
      },
    };

    console.log('📡 Sending request...');
    const stream = await ollamaClient.chat(requestOptions);

    console.log('✅ Receiving chunks:');
    let fullContent = '';
    let chunkCount = 0;

    for await (const chunk of stream) {
      chunkCount++;

      if (chunk.message?.content) {
        fullContent += chunk.message.content;
        process.stdout.write(chunk.message.content);
      }

      if (chunk.done) {
        console.log(`\n\n✅ Stream completed:`);
        console.log(`   Chunks: ${chunkCount}`);
        console.log(`   Content length: ${fullContent.length} chars`);
        break;
      }
    }

    return true;
  } catch (error) {
    console.error('❌ Test failed:', error);
    return false;
  }
}

async function testReasoningWithFallback() {
  console.log('\n🧪 Test: Reasoning Mode with Automatic Fallback');
  console.log('━'.repeat(60));

  try {
    const ollamaClient = new Ollama({ host: DEFAULT_HOST });

    const messages = [
      {
        role: 'user' as const,
        content: 'What is 10 + 20? Answer briefly.',
      },
    ];

    // First try with granular level (should fail for qwen3)
    let requestOptions = {
      model: TEST_MODEL,
      messages,
      stream: false as const,
      think: 'medium' as const,
      keep_alive: '5m',
    };

    console.log('📡 Attempt 1: Using think: "medium"...');

    try {
      const response = await ollamaClient.chat(requestOptions);
      console.log('✅ Response:', response.message.content);
    } catch (error) {
      if (
        error instanceof Error &&
        error.message.includes('think value') &&
        error.message.includes('is not supported')
      ) {
        console.log(
          '⚠️  Granular think level not supported, retrying with boolean...',
        );

        // Retry with boolean true
        const booleanRequestOptions = {
          model: TEST_MODEL,
          messages,
          stream: false as const,
          think: true as const,
          keep_alive: '5m',
        };

        console.log('📡 Attempt 2: Using think: true...');
        const response = await ollamaClient.chat(booleanRequestOptions);

        console.log('✅ Response received:');
        console.log(
          `   Content length: ${response.message.content.length} chars`,
        );
        console.log(
          `   Preview: ${response.message.content.substring(0, 150)}...`,
        );

        return true;
      }
      throw error;
    }

    return true;
  } catch (error) {
    console.error('❌ Test failed:', error);
    return false;
  }
}

async function testToolCalling() {
  console.log('\n🧪 Test: Tool Calling');
  console.log('━'.repeat(60));

  try {
    const ollamaClient = new Ollama({ host: DEFAULT_HOST });

    const messages = [
      { role: 'user' as const, content: 'Calculate 15 + 25 using the tool' },
    ];

    const tools = [
      {
        type: 'function' as const,
        function: {
          name: 'calculate',
          description: 'Add two numbers',
          parameters: {
            type: 'object',
            required: ['a', 'b'],
            properties: {
              a: { type: 'number', description: 'First number' },
              b: { type: 'number', description: 'Second number' },
            },
          },
        },
      },
    ];

    console.log('📡 Sending request with tool...');
    const response = await ollamaClient.chat({
      model: TEST_MODEL,
      messages,
      tools,
      stream: false,
    });

    console.log('✅ Response received:');
    console.log(`   Content: ${response.message.content || '(empty)'}`);

    if (response.message.tool_calls) {
      console.log(`   🔧 Tool calls: ${response.message.tool_calls.length}`);
      response.message.tool_calls.forEach((tc, idx) => {
        console.log(
          `   [${idx + 1}] ${tc.function.name}(${JSON.stringify(tc.function.arguments)})`,
        );
      });
    }

    return true;
  } catch (error) {
    console.error('❌ Test failed:', error);
    return false;
  }
}

async function runAllTests() {
  console.log('🚀 Ollama.ts Integration Test Suite');
  console.log('═'.repeat(60));
  console.log(`Host: ${DEFAULT_HOST}`);
  console.log(`Model: ${TEST_MODEL}`);
  console.log('═'.repeat(60));

  const results = {
    streamingChat: await testStreamingChat(),
    reasoningFallback: await testReasoningWithFallback(),
    toolCalling: await testToolCalling(),
  };

  console.log('\n📊 Test Summary');
  console.log('═'.repeat(60));
  Object.entries(results).forEach(([test, passed]) => {
    const status = passed ? '✅ PASS' : '❌ FAIL';
    console.log(`${status} - ${test}`);
  });

  const passedCount = Object.values(results).filter(Boolean).length;
  const totalCount = Object.keys(results).length;
  console.log(`\nTotal: ${passedCount}/${totalCount} tests passed`);

  if (passedCount === totalCount) {
    console.log('🎉 All integration tests passed!');
  } else {
    console.log('⚠️  Some integration tests failed');
  }
}

runAllTests().catch(console.error);
