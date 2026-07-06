/**
 * Simplified test for context window detection
 * Directly tests the fetch functions without using internal loggers
 */

import { AIServiceProvider } from '../src/lib/ai-service/types';

async function testContextWindow() {
  console.log('\n╔═══════════════════════════════════════════════════╗');
  console.log('║   Context Window Detection - Quick Test          ║');
  console.log('╚═══════════════════════════════════════════════════╝\n');

  // Test 1: Direct OpenRouter API fetch
  console.log('🧪 Test 1: OpenRouter API - Direct Fetch');
  console.log('─────────────────────────────────────────────────────');

  try {
    const response = await fetch('https://openrouter.ai/api/v1/models');
    const data = await response.json();

    console.log(`✅ Fetched ${data.data.length} models from OpenRouter`);

    // Check a few models
    const testModels = [
      'openai/gpt-4o',
      'anthropic/claude-opus-4',
      'google/gemini-2.5-pro',
    ];

    for (const modelId of testModels) {
      const model = data.data.find((m: { id: string }) => m.id === modelId);
      if (model) {
        console.log(
          `   ${modelId}: ${model.context_length?.toLocaleString()} tokens`,
        );
      } else {
        console.log(`   ${modelId}: Not found`);
      }
    }
  } catch (error) {
    console.error('❌ OpenRouter API test failed:', error);
  }

  // Test 2: Ollama API test (if running)
  console.log('\n🧪 Test 2: Ollama API - /api/show');
  console.log('─────────────────────────────────────────────────────');

  try {
    const tagsResponse = await fetch('http://localhost:11434/api/tags');
    const tagsData = await tagsResponse.json();

    if (tagsData.models && tagsData.models.length > 0) {
      console.log(`✅ Ollama running with ${tagsData.models.length} models`);

      // Test first model
      const testModel = tagsData.models[0].name;
      console.log(`   Testing: ${testModel}`);

      const showResponse = await fetch('http://localhost:11434/api/show', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: testModel }),
      });

      const showData = await showResponse.json();
      const contextWindow =
        showData.parameters?.num_ctx ||
        showData.model_info?.['llama.context_length'];

      console.log(
        `   Context Window: ${contextWindow?.toLocaleString() || 'N/A'} tokens`,
      );
      console.log(
        `   Has think param: ${showData.modelfile?.includes('think') || false}`,
      );
    } else {
      console.log('⚠️  No Ollama models installed');
    }
  } catch (error) {
    console.log('⚠️  Ollama not running or not accessible');
  }

  // Test 3: estimateContextWindow fallback
  console.log('\n🧪 Test 3: Fallback Heuristics');
  console.log('─────────────────────────────────────────────────────');

  const { estimateContextWindow } =
    await import('../src/lib/ai-service/model-capabilities');

  const testCases = [
    { provider: AIServiceProvider.OpenAI, model: 'gpt-4o', expected: 128000 },
    { provider: AIServiceProvider.OpenAI, model: 'gpt-4', expected: 32768 },
    {
      provider: AIServiceProvider.Anthropic,
      model: 'claude-3',
      expected: 200000,
    },
    { provider: AIServiceProvider.Ollama, model: 'llama3.1', expected: 128000 },
    {
      provider: AIServiceProvider.Gemini,
      model: 'gemini-1.5-pro',
      expected: 1000000,
    },
  ];

  for (const test of testCases) {
    const result = estimateContextWindow(test.model, test.provider);
    const match = result === test.expected ? '✅' : '⚠️';
    console.log(
      `   ${match} ${test.provider}/${test.model}: ${result.toLocaleString()} (expected: ${test.expected.toLocaleString()})`,
    );
  }

  console.log('\n✅ Quick test completed!');
  console.log('\n💡 To test full integration:');
  console.log('   1. Start the Tauri app: pnpm tauri dev');
  console.log('   2. Open the model picker');
  console.log('   3. Check context window values in UI');
}

testContextWindow().catch(console.error);
