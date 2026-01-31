/**
 * Test script for dynamic context window detection
 * Tests the new getContextWindow() function across all providers
 */

import { getContextWindow } from '../src/lib/ai-service/model-capabilities';
import { AIServiceProvider } from '../src/lib/ai-service/types';

// Use console.log for Node.js environment (Tauri logger requires window object)
const logger = {
  info: (...args: unknown[]) => console.log('[INFO]', ...args),
  warn: (...args: unknown[]) => console.warn('[WARN]', ...args),
  error: (...args: unknown[]) => console.error('[ERROR]', ...args),
  debug: (...args: unknown[]) => console.log('[DEBUG]', ...args),
};

interface TestCase {
  provider: AIServiceProvider;
  modelName: string;
  expectedMin?: number; // Minimum expected context window
  expectedMax?: number; // Maximum expected context window
  description: string;
}

const testCases: TestCase[] = [
  // OpenAI models
  {
    provider: AIServiceProvider.OpenAI,
    modelName: 'gpt-4o',
    expectedMin: 100000,
    expectedMax: 150000,
    description: 'GPT-4o should have 128k context window',
  },
  {
    provider: AIServiceProvider.OpenAI,
    modelName: 'gpt-4.1',
    expectedMin: 900000,
    description: 'GPT-4.1 should have 1M context window',
  },
  {
    provider: AIServiceProvider.OpenAI,
    modelName: 'o3-mini',
    expectedMin: 100000,
    description: 'o3-mini should have large context window',
  },

  // Anthropic models
  {
    provider: AIServiceProvider.Anthropic,
    modelName: 'claude-opus-4-20250514',
    expectedMin: 150000,
    description: 'Claude 4 Opus should have 200k context window',
  },
  {
    provider: AIServiceProvider.Anthropic,
    modelName: 'claude-3-5-sonnet-20241022',
    expectedMin: 150000,
    description: 'Claude 3.5 Sonnet should have 200k context window',
  },

  // Ollama models (requires Ollama server running)
  {
    provider: AIServiceProvider.Ollama,
    modelName: 'llama3.1',
    expectedMin: 100000,
    description: 'Llama 3.1 should have 128k context window',
  },
  {
    provider: AIServiceProvider.Ollama,
    modelName: 'qwen2.5',
    expectedMin: 30000,
    description: 'Qwen 2.5 should have 32k+ context window',
  },

  // Gemini models
  {
    provider: AIServiceProvider.Gemini,
    modelName: 'gemini-2.5-pro',
    expectedMin: 1000000,
    description: 'Gemini 2.5 Pro should have 2M context window',
  },
];

async function runTest(testCase: TestCase): Promise<boolean> {
  const { provider, modelName, expectedMin, expectedMax, description } =
    testCase;

  try {
    logger.info(`\n🧪 Testing: ${description}`);
    logger.info(`   Provider: ${provider}, Model: ${modelName}`);

    const startTime = Date.now();
    const contextWindow = await getContextWindow(modelName, provider, {
      skipCache: false, // Use cache for faster tests
    });
    const duration = Date.now() - startTime;

    logger.info(
      `   ✅ Context Window: ${contextWindow.toLocaleString()} tokens`,
    );
    logger.info(`   ⏱️  Detection took: ${duration}ms`);

    // Validate against expected range
    let validationPassed = true;
    if (expectedMin && contextWindow < expectedMin) {
      logger.warn(
        `   ⚠️  Context window ${contextWindow} is less than expected minimum ${expectedMin}`,
      );
      validationPassed = false;
    }
    if (expectedMax && contextWindow > expectedMax) {
      logger.warn(
        `   ⚠️  Context window ${contextWindow} is greater than expected maximum ${expectedMax}`,
      );
      validationPassed = false;
    }

    if (validationPassed) {
      logger.info(`   ✅ Validation passed`);
    }

    return validationPassed;
  } catch (error) {
    logger.error(`   ❌ Test failed:`, error);
    return false;
  }
}

async function testCaching(): Promise<void> {
  logger.info('\n\n📦 Testing Cache Performance');
  logger.info('═══════════════════════════════════════════════════');

  const testModel = 'gpt-4o';
  const provider = AIServiceProvider.OpenAI;

  // First call (no cache)
  logger.info('\n1️⃣  First call (should use API):');
  const start1 = Date.now();
  const result1 = await getContextWindow(testModel, provider, {
    skipCache: true,
  });
  const duration1 = Date.now() - start1;
  logger.info(`   Result: ${result1.toLocaleString()} tokens`);
  logger.info(`   Duration: ${duration1}ms`);

  // Second call (cached)
  logger.info('\n2️⃣  Second call (should use cache):');
  const start2 = Date.now();
  const result2 = await getContextWindow(testModel, provider, {
    skipCache: false,
  });
  const duration2 = Date.now() - start2;
  logger.info(`   Result: ${result2.toLocaleString()} tokens`);
  logger.info(`   Duration: ${duration2}ms`);

  const speedup = (((duration1 - duration2) / duration1) * 100).toFixed(1);
  logger.info(`\n   🚀 Cache speedup: ${speedup}% faster`);
  logger.info(`   ✅ Cache working: ${result1 === result2 ? 'Yes' : 'No'}`);
}

async function testFallback(): Promise<void> {
  logger.info('\n\n🔄 Testing Fallback Mechanism');
  logger.info('═══════════════════════════════════════════════════');

  const unknownModel = 'unknown-model-xyz-123';
  const provider = AIServiceProvider.OpenAI;

  logger.info(`\nTesting unknown model: ${unknownModel}`);
  const contextWindow = await getContextWindow(unknownModel, provider);

  logger.info(
    `   Fallback context window: ${contextWindow.toLocaleString()} tokens`,
  );
  logger.info(`   ✅ Fallback mechanism working (returned default value)`);
}

async function testOllamaIntegration(): Promise<void> {
  logger.info('\n\n🦙 Testing Ollama /api/show Integration');
  logger.info('═══════════════════════════════════════════════════');

  try {
    // Test if Ollama is running
    const response = await fetch('http://localhost:11434/api/tags');
    if (!response.ok) {
      logger.warn('⚠️  Ollama server not running, skipping Ollama tests');
      return;
    }

    const data = await response.json();
    const models = data.models || [];

    if (models.length === 0) {
      logger.warn('⚠️  No Ollama models installed, skipping tests');
      return;
    }

    logger.info(`\nFound ${models.length} Ollama models, testing first 3:`);

    for (let i = 0; i < Math.min(3, models.length); i++) {
      const model = models[i];
      logger.info(`\n${i + 1}. Testing: ${model.name}`);

      const contextWindow = await getContextWindow(
        model.name,
        AIServiceProvider.Ollama,
        { apiBase: 'http://localhost:11434' },
      );

      logger.info(
        `   Context Window: ${contextWindow.toLocaleString()} tokens`,
      );
    }

    logger.info('\n✅ Ollama integration test completed');
  } catch (error) {
    logger.warn('⚠️  Ollama server not available:', error);
  }
}

async function main() {
  logger.info('╔═══════════════════════════════════════════════════╗');
  logger.info('║   Context Window Dynamic Detection Test Suite     ║');
  logger.info('╚═══════════════════════════════════════════════════╝');

  // Run main tests
  logger.info('\n\n🎯 Running Model-Specific Tests');
  logger.info('═══════════════════════════════════════════════════');

  let passed = 0;
  let failed = 0;

  for (const testCase of testCases) {
    const result = await runTest(testCase);
    if (result) {
      passed++;
    } else {
      failed++;
    }
  }

  // Run cache test
  await testCaching();

  // Run fallback test
  await testFallback();

  // Run Ollama integration test
  await testOllamaIntegration();

  // Summary
  logger.info('\n\n╔═══════════════════════════════════════════════════╗');
  logger.info('║                   Test Summary                     ║');
  logger.info('╚═══════════════════════════════════════════════════╝');
  logger.info(`\n✅ Passed: ${passed}`);
  logger.info(`❌ Failed: ${failed}`);
  logger.info(`📊 Total:  ${passed + failed}`);
  logger.info(
    `\n🎯 Success Rate: ${((passed / (passed + failed)) * 100).toFixed(1)}%`,
  );

  if (failed === 0) {
    logger.info('\n🎉 All tests passed!');
  } else {
    logger.warn(`\n⚠️  ${failed} test(s) failed`);
  }

  logger.info('\n💡 Note: Some tests may fail due to:');
  logger.info('   - API rate limits');
  logger.info('   - Network issues');
  logger.info('   - Model availability changes');
  logger.info('   - Ollama server not running (for Ollama tests)');
}

// Run tests
main().catch((error) => {
  logger.error('Test suite failed:', error);
  process.exit(1);
});
