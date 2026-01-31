/**
 * Test script to verify reasoning mode implementations across providers
 * Tests: Ollama (think), Anthropic (extended_thinking), OpenAI (reasoning_effort)
 */

import { OllamaService } from '../src/lib/ai-service/ollama';
import { AnthropicService } from '../src/lib/ai-service/anthropic';
import { OpenAIService } from '../src/lib/ai-service/openai';
import { Message } from '../src/models/chat';
import { AIServiceConfig } from '../src/lib/ai-service/types';

// Simple test message
const testMessages: Message[] = [
  {
    id: 'test-1',
    sessionId: 'test-session',
    threadId: 'test-session',
    role: 'user',
    content: [
      {
        type: 'text',
        text: 'What is 15 * 24? Please show your reasoning step by step.',
      },
    ],
  },
];

async function testOllamaReasoning() {
  console.log('\n=== Testing Ollama Reasoning Mode ===\n');

  const service = new OllamaService('ollama-local', {
    host: 'http://127.0.0.1:11434',
  });

  const config: AIServiceConfig = {
    enableReasoning: true,
    reasoningEffort: 'medium', // Will fallback to boolean true
    maxTokens: 2000,
    temperature: 0.7,
  };

  try {
    console.log('Streaming with reasoning enabled (qwen3:8b)...\n');
    let thinkingContent = '';
    let regularContent = '';

    for await (const chunk of service.streamChat(testMessages, {
      modelName: 'qwen3:8b',
      config,
    })) {
      const parsed = JSON.parse(chunk);
      if (parsed.thinking) {
        thinkingContent += parsed.thinking;
        process.stdout.write(`💭 ${parsed.thinking}`);
      }
      if (parsed.content) {
        regularContent += parsed.content;
        process.stdout.write(parsed.content);
      }
    }

    console.log('\n\n--- Results ---');
    console.log(`Thinking length: ${thinkingContent.length} chars`);
    console.log(`Content length: ${regularContent.length} chars`);
    console.log(`Has thinking: ${thinkingContent.length > 0 ? '✅' : '❌'}`);
  } catch (error) {
    console.error('❌ Ollama test failed:', error);
  }

  service.dispose();
}

async function testAnthropicReasoning() {
  console.log('\n=== Testing Anthropic Reasoning Mode ===\n');

  const apiKey = process.env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    console.log('⏭️  Skipping: ANTHROPIC_API_KEY not set');
    return;
  }

  const service = new AnthropicService(apiKey);

  const config: AIServiceConfig = {
    enableReasoning: true, // Will use extended_thinking for Claude 3.5+
    maxTokens: 2000,
    temperature: 0.7,
  };

  try {
    console.log('Streaming with extended_thinking (claude-opus-4)...\n');
    let thinkingContent = '';
    let regularContent = '';

    for await (const chunk of service.streamChat(testMessages, {
      modelName: 'claude-opus-4-20250514',
      config,
    })) {
      const parsed = JSON.parse(chunk);
      if (parsed.thinking) {
        thinkingContent += JSON.stringify(parsed.thinking);
        console.log('💭 Thinking:', parsed.thinking);
      }
      if (parsed.content) {
        regularContent += parsed.content;
        process.stdout.write(parsed.content);
      }
    }

    console.log('\n\n--- Results ---');
    console.log(
      `Thinking detected: ${thinkingContent.length > 0 ? '✅' : '❌'}`,
    );
    console.log(`Content length: ${regularContent.length} chars`);
  } catch (error) {
    console.error('❌ Anthropic test failed:', error);
  }

  service.dispose();
}

async function testOpenAIReasoning() {
  console.log('\n=== Testing OpenAI Reasoning Mode ===\n');

  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) {
    console.log('⏭️  Skipping: OPENAI_API_KEY not set');
    return;
  }

  const service = new OpenAIService(apiKey);

  const config: AIServiceConfig = {
    enableReasoning: true,
    reasoningEffort: 'medium', // For o3/o4 models
    maxTokens: 2000,
    temperature: 0.7,
  };

  try {
    console.log('Streaming with reasoning_effort (o4-mini)...\n');
    let regularContent = '';

    for await (const chunk of service.streamChat(testMessages, {
      modelName: 'o4-mini',
      config,
    })) {
      const parsed = JSON.parse(chunk);
      if (parsed.content) {
        regularContent += parsed.content;
        process.stdout.write(parsed.content);
      }
    }

    console.log('\n\n--- Results ---');
    console.log(`Content length: ${regularContent.length} chars`);
    console.log(
      `Response generated: ${regularContent.length > 0 ? '✅' : '❌'}`,
    );
  } catch (error) {
    console.error('❌ OpenAI test failed:', error);
  }

  service.dispose();
}

async function main() {
  console.log('🧪 Testing Reasoning Modes Across Providers\n');
  console.log('This script tests:');
  console.log('  - Ollama: think parameter + thinking field extraction');
  console.log('  - Anthropic: extended_thinking parameter for Claude 3.5+');
  console.log('  - OpenAI: reasoning_effort parameter for o3/o4 models\n');

  await testOllamaReasoning();
  await testAnthropicReasoning();
  await testOpenAIReasoning();

  console.log('\n✅ All tests completed\n');
}

main().catch(console.error);
