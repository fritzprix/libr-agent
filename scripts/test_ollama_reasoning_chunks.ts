#!/usr/bin/env tsx
/**
 * Test Ollama with reasoning mode to see actual chunk structure
 */

import { Ollama } from 'ollama/browser';

const DEFAULT_HOST = 'http://127.0.0.1:11434';
const TEST_MODEL = 'qwen3:8b';

async function testReasoningChunks() {
  console.log('🧪 Testing Ollama Reasoning Mode Chunks');
  console.log('═'.repeat(60));

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    console.log(`📡 Request with think: true`);
    console.log(`🤖 Model: ${TEST_MODEL}\n`);

    const stream = await ollama.chat({
      model: TEST_MODEL,
      messages: [{ role: 'user', content: 'Say hello briefly' }],
      stream: true,
      think: true, // Enable reasoning
    });

    console.log('📦 Analyzing chunks:\n');
    let chunkIndex = 0;
    let totalContentLength = 0;
    let emptyChunks = 0;

    for await (const chunk of stream) {
      chunkIndex++;

      const hasMessage =
        chunk && typeof chunk === 'object' && 'message' in chunk;
      const message =
        hasMessage && chunk.message && typeof chunk.message === 'object'
          ? (chunk.message as unknown as Record<string, unknown>)
          : null;
      const hasContent = message && 'content' in message;
      const hasThinking = message && 'thinking' in message;
      const contentValue =
        hasContent && typeof message.content === 'string'
          ? message.content
          : '';
      const thinkingValue =
        hasThinking && typeof message.thinking === 'string'
          ? message.thinking
          : '';
      const contentLength = contentValue.length;
      const thinkingLength = thinkingValue.length;

      if (contentLength === 0 && thinkingLength === 0) {
        emptyChunks++;
      } else {
        totalContentLength += contentLength + thinkingLength;
      }

      // Log first 5 chunks in detail
      if (chunkIndex <= 5) {
        console.log(`Chunk #${chunkIndex}:`);
        console.log(`  Has message: ${hasMessage}`);
        console.log(`  Has content: ${hasContent}`);
        console.log(`  Content length: ${contentLength}`);
        console.log(`  Has thinking: ${hasThinking}`);
        console.log(`  Thinking length: ${thinkingLength}`);
        if (contentValue && contentLength > 0) {
          console.log(
            `  Content preview: "${contentValue.substring(0, 50)}${contentLength > 50 ? '...' : ''}"`,
          );
        }
        if (thinkingValue && thinkingLength > 0) {
          console.log(
            `  Thinking preview: "${thinkingValue.substring(0, 50)}${thinkingLength > 50 ? '...' : ''}"`,
          );
        }
        console.log(
          `  Chunk keys: ${chunk ? Object.keys(chunk).join(', ') : 'N/A'}`,
        );
        if (message) {
          console.log(`  Message keys: ${Object.keys(message).join(', ')}`);
        }
        console.log();
      }

      if (chunk.done) {
        console.log(`\n✅ Stream completed:`);
        console.log(`   Total chunks: ${chunkIndex}`);
        console.log(`   Empty chunks: ${emptyChunks}`);
        console.log(`   Chunks with data: ${chunkIndex - emptyChunks}`);
        console.log(`   Total data length: ${totalContentLength} chars`);
        break;
      }
    }
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

testReasoningChunks();
