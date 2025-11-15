#!/usr/bin/env tsx
/**
 * Q2 테스트 스크립트: OpenRouter API 접근 가능성 확인
 *
 * 테스트 항목:
 * 1. /api/v1/models 엔드포인트가 API key 없이 접근 가능한지
 * 2. 응답 데이터에 reasoning 관련 필드가 포함되어 있는지
 * 3. 주요 reasoning 모델들의 메타데이터 확인
 */

interface OpenRouterModel {
  id: string;
  name: string;
  pricing?: {
    prompt: string;
    completion: string;
  };
  context_length?: number;
  architecture?: {
    modality?: string;
    tokenizer?: string;
    instruct_type?: string;
  };
  top_provider?: {
    max_completion_tokens?: number;
  };
  per_request_limits?: {
    prompt_tokens?: string;
    completion_tokens?: string;
  };
  supported_parameters?: string[];
}

interface OpenRouterModelsResponse {
  data: OpenRouterModel[];
}

const OPENROUTER_API_BASE = 'https://openrouter.ai/api/v1';
const REASONING_MODEL_SAMPLES = [
  'openai/o1-preview',
  'openai/o1-mini',
  'openai/o3-mini',
  'anthropic/claude-3.5-sonnet',
  'google/gemini-2.5-flash',
  'deepseek/deepseek-r1',
  'qwen/qwen-2.5-72b-instruct',
];

async function testOpenRouterAccess() {
  console.log('🧪 OpenRouter API 접근 테스트 시작\n');

  // Test 1: API key 없이 접근 가능한지
  console.log('📡 Test 1: /api/v1/models 엔드포인트 접근 (API key 없음)');
  try {
    const startTime = Date.now();
    const response = await fetch(`${OPENROUTER_API_BASE}/models`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        // No Authorization header - testing public access
      },
    });

    const elapsed = Date.now() - startTime;
    console.log(`✅ 응답 상태: ${response.status} ${response.statusText}`);
    console.log(`⏱️  응답 시간: ${elapsed}ms`);

    if (!response.ok) {
      console.error(`❌ API 접근 실패: ${response.status}`);
      const errorText = await response.text();
      console.error(`에러 내용: ${errorText}`);
      return;
    }

    const data: OpenRouterModelsResponse = await response.json();
    console.log(`✅ 총 모델 수: ${data.data.length}개\n`);

    // Test 2: reasoning 관련 필드 확인
    console.log('🔍 Test 2: Reasoning 관련 필드 분석');

    const modelsWithReasoningParam = data.data.filter((model) =>
      model.supported_parameters?.includes('reasoning'),
    );

    const modelsWithThinkingParam = data.data.filter((model) =>
      model.supported_parameters?.includes('thinking'),
    );

    console.log(
      `✅ 'reasoning' 파라미터 지원 모델: ${modelsWithReasoningParam.length}개`,
    );
    console.log(
      `✅ 'thinking' 파라미터 지원 모델: ${modelsWithThinkingParam.length}개\n`,
    );

    // Test 3: 주요 reasoning 모델 메타데이터 확인
    console.log('🎯 Test 3: 주요 Reasoning 모델 메타데이터\n');

    for (const sampleModelId of REASONING_MODEL_SAMPLES) {
      const model = data.data.find((m) => m.id === sampleModelId);

      if (model) {
        console.log(`📦 모델: ${model.id}`);
        console.log(`   이름: ${model.name}`);
        console.log(
          `   지원 파라미터: ${model.supported_parameters?.join(', ') || 'N/A'}`,
        );
        console.log(
          `   컨텍스트 길이: ${model.context_length?.toLocaleString() || 'N/A'}`,
        );

        if (model.pricing) {
          console.log(`   비용 (입력): $${model.pricing.prompt}/1M tokens`);
          console.log(`   비용 (출력): $${model.pricing.completion}/1M tokens`);
        }

        const hasReasoning = model.supported_parameters?.includes('reasoning');
        const hasThinking = model.supported_parameters?.includes('thinking');
        console.log(`   ${hasReasoning ? '✅' : '❌'} reasoning 지원`);
        console.log(`   ${hasThinking ? '✅' : '❌'} thinking 지원`);
        console.log();
      } else {
        console.log(`⚠️  모델 '${sampleModelId}'를 찾을 수 없음\n`);
      }
    }

    // Summary
    console.log('📊 요약');
    console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
    console.log(`✅ API key 없이 접근 가능: YES`);
    console.log(`✅ 총 모델 수: ${data.data.length}개`);
    console.log(`✅ Reasoning 지원 모델: ${modelsWithReasoningParam.length}개`);
    console.log(`✅ Thinking 지원 모델: ${modelsWithThinkingParam.length}개`);

    // 첫 3개 reasoning 모델 출력
    if (modelsWithReasoningParam.length > 0) {
      console.log(`\n🧠 Reasoning 지원 모델 예시 (처음 5개):`);
      modelsWithReasoningParam.slice(0, 5).forEach((model, idx) => {
        console.log(`   ${idx + 1}. ${model.id}`);
      });
    }

    if (modelsWithThinkingParam.length > 0) {
      console.log(`\n💭 Thinking 지원 모델 예시 (처음 5개):`);
      modelsWithThinkingParam.slice(0, 5).forEach((model, idx) => {
        console.log(`   ${idx + 1}. ${model.id}`);
      });
    }

    console.log('\n✅ Q2 결론: OpenRouter API는 인증 없이 접근 가능하며,');
    console.log(
      '   모델 메타데이터에 reasoning/thinking 파라미터 정보가 포함되어 있습니다.',
    );
  } catch (error) {
    console.error('❌ 테스트 실패:', error);
    if (error instanceof Error) {
      console.error(`   에러 메시지: ${error.message}`);
      console.error(`   스택: ${error.stack}`);
    }
  }
}

// 스크립트 실행
testOpenRouterAccess();
