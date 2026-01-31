#!/usr/bin/env tsx
/**
 * Quick diagnostic to check current Ollama state
 */

import { Ollama } from 'ollama/browser';

const DEFAULT_HOST = 'http://127.0.0.1:11434';

async function checkOllamaState() {
  console.log('🔍 Checking Ollama State');
  console.log('═'.repeat(60));

  try {
    const ollama = new Ollama({ host: DEFAULT_HOST });

    // List models
    console.log('\n📋 Available Models:');
    const models = await ollama.list();
    models.models.forEach((m, idx) => {
      console.log(`  ${idx + 1}. ${m.name}`);
    });

    // Try a simple chat with first available model
    if (models.models.length > 0) {
      const testModel = models.models[0].name;
      console.log(`\n🧪 Testing with: ${testModel}`);

      const response = await ollama.chat({
        model: testModel,
        messages: [{ role: 'user', content: 'Say hello' }],
        stream: false,
      });

      console.log('✅ Response:', response.message.content);
    }
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

checkOllamaState();
