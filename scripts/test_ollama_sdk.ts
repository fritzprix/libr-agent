#!/usr/bin/env tsx
/**
 * Ollama SDK 동작 검증 테스트 스크립트
 *
 * 목적:
 * 1. Ollama SDK의 기본 chat 기능 확인
 * 2. Streaming 응답 확인
 * 3. Tool calling 확인
 * 4. Browser 환경 import 확인
 */

import { Ollama } from 'ollama/browser';

const DEFAULT_HOST = 'http://127.0.0.1:11434';
const TEST_MODEL = 'qwen3:8b'; // Changed from qwen2.5:latest to available model

async function test1_BasicChat() {
  console.log('\n🧪 Test 1: Basic Chat (Non-streaming)');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Sending request to ${DEFAULT_HOST}`);
    console.log(`🤖 Model: ${TEST_MODEL}`);
    console.log(`💬 Message: "Hello, how are you?"\n`);

    const response = await ollama.chat({
      model: TEST_MODEL,
      messages: [{ role: 'user', content: 'Hello, how are you?' }],
      stream: false,
    });

    console.log('✅ Response received:');
    console.log(`   Role: ${response.message.role}`);
    console.log(`   Content: ${response.message.content}`);
    console.log(`   Done: ${response.done}`);
    console.log(`   Model: ${response.model}`);

    if (response.total_duration) {
      console.log(`   Duration: ${response.total_duration / 1_000_000}ms`);
    }

    return true;
  } catch (error) {
    console.error('❌ Test 1 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
      console.error(`   Stack: ${error.stack}`);
    }
    return false;
  }
}

async function test2_StreamingChat() {
  console.log('\n🧪 Test 2: Streaming Chat');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Sending streaming request to ${DEFAULT_HOST}`);
    console.log(`🤖 Model: ${TEST_MODEL}`);
    console.log(`💬 Message: "Count from 1 to 5"\n`);

    const stream = await ollama.chat({
      model: TEST_MODEL,
      messages: [{ role: 'user', content: 'Count from 1 to 5' }],
      stream: true,
    });

    console.log('✅ Streaming chunks:');
    let chunkCount = 0;
    let fullContent = '';

    for await (const chunk of stream) {
      chunkCount++;
      if (chunk.message?.content) {
        fullContent += chunk.message.content;
        process.stdout.write(chunk.message.content);
      }

      if (chunk.done) {
        console.log(`\n\n   Total chunks: ${chunkCount}`);
        console.log(`   Full content length: ${fullContent.length} chars`);
        console.log(`   Done: ${chunk.done}`);
        if (chunk.total_duration) {
          console.log(`   Duration: ${chunk.total_duration / 1_000_000}ms`);
        }
      }
    }

    return true;
  } catch (error) {
    console.error('\n❌ Test 2 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
    }
    return false;
  }
}

async function test3_ToolCalling() {
  console.log('\n🧪 Test 3: Tool Calling');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    const addTool = {
      type: 'function' as const,
      function: {
        name: 'addTwoNumbers',
        description: 'Add two numbers together',
        parameters: {
          type: 'object',
          required: ['a', 'b'],
          properties: {
            a: { type: 'number', description: 'The first number' },
            b: { type: 'number', description: 'The second number' },
          },
        },
      },
    };

    console.log(`📡 Sending request with tool`);
    console.log(`🤖 Model: ${TEST_MODEL}`);
    console.log(`💬 Message: "What is 5 plus 3?"`);
    console.log(`🔧 Tool: addTwoNumbers\n`);

    const response = await ollama.chat({
      model: TEST_MODEL,
      messages: [{ role: 'user', content: 'What is 5 plus 3?' }],
      tools: [addTool],
      stream: false,
    });

    console.log('✅ Response received:');
    console.log(`   Role: ${response.message.role}`);
    console.log(`   Content: ${response.message.content || '(empty)'}`);

    if (response.message.tool_calls) {
      console.log(`\n   🔧 Tool Calls: ${response.message.tool_calls.length}`);
      response.message.tool_calls.forEach((tc, idx) => {
        console.log(
          `   [${idx + 1}] ${tc.function.name}(${JSON.stringify(tc.function.arguments)})`,
        );
      });
    } else {
      console.log(`   🔧 Tool Calls: None`);
    }

    return true;
  } catch (error) {
    console.error('❌ Test 3 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
    }
    return false;
  }
}

async function test4_ReasoningMode() {
  console.log('\n🧪 Test 4: Reasoning Mode (think parameter)');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Sending request with think: 'medium'`);
    console.log(`🤖 Model: ${TEST_MODEL}`);
    console.log(`💬 Message: "Explain quantum computing briefly"\n`);

    const response = await ollama.chat({
      model: TEST_MODEL,
      messages: [
        { role: 'user', content: 'Explain quantum computing briefly' },
      ],
      think: 'medium', // or true, 'low', 'high'
      stream: false,
    });

    console.log('✅ Response received:');
    console.log(`   Role: ${response.message.role}`);
    console.log(`   Content length: ${response.message.content.length} chars`);
    console.log(
      `   Content preview: ${response.message.content.substring(0, 100)}...`,
    );

    return true;
  } catch (error) {
    console.error('❌ Test 4 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
    }
    return false;
  }
}

async function test5_ListModels() {
  console.log('\n🧪 Test 5: List Available Models');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Fetching model list from ${DEFAULT_HOST}\n`);

    const modelList = await ollama.list();

    console.log(`✅ Found ${modelList.models.length} models:`);
    modelList.models.forEach((model, idx) => {
      console.log(`   [${idx + 1}] ${model.name}`);
      console.log(
        `       Size: ${(model.size / 1024 / 1024 / 1024).toFixed(2)} GB`,
      );
      console.log(
        `       Modified: ${new Date(model.modified_at).toLocaleString()}`,
      );
    });

    return true;
  } catch (error) {
    console.error('❌ Test 5 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
    }
    return false;
  }
}

async function test6_AbortControl() {
  console.log('\n🧪 Test 6: Abort Control');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Starting long streaming request`);
    console.log(`⏱️  Will abort after 2 seconds\n`);

    const stream = await ollama.chat({
      model: TEST_MODEL,
      messages: [
        {
          role: 'user',
          content: 'Write a very long story about space exploration',
        },
      ],
      stream: true,
    });

    // Abort after 2 seconds
    setTimeout(() => {
      console.log('\n🛑 Aborting stream...');
      ollama.abort();
    }, 2000);

    let chunkCount = 0;
    try {
      for await (const chunk of stream) {
        chunkCount++;
        if (chunk.message?.content) {
          process.stdout.write(chunk.message.content);
        }
      }
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        console.log(
          `\n✅ Stream aborted successfully after ${chunkCount} chunks`,
        );
        return true;
      }
      throw error;
    }

    console.log('\n⚠️  Stream completed without abort');
    return false;
  } catch (error) {
    console.error('\n❌ Test 6 Failed:', error);
    if (error instanceof Error) {
      console.error(`   Message: ${error.message}`);
    }
    return false;
  }
}

// Main test runner
async function runAllTests() {
  console.log('🚀 Ollama SDK Test Suite');
  console.log('═'.repeat(60));
  console.log(`Host: ${DEFAULT_HOST}`);
  console.log(`Model: ${TEST_MODEL}`);
  console.log('═'.repeat(60));

  const results = {
    test1: await test1_BasicChat(),
    test2: await test2_StreamingChat(),
    test3: await test3_ToolCalling(),
    test4: await test4_ReasoningMode(),
    test5: await test5_ListModels(),
    test6: await test6_AbortControl(),
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
    console.log('🎉 All tests passed!');
  } else {
    console.log('⚠️  Some tests failed');
  }
}

// Run tests
runAllTests().catch(console.error);
