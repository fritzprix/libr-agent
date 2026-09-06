import type { SolutionRecipe } from './types';

export const MORNING_BRIEFING_RECIPE: SolutionRecipe = {
  id: 'morning-briefing',
  titleKey: 'recipes.morningBriefing.title',
  defaultTitle: '모닝 테크 & 금융 브리핑',
  descKey: 'recipes.morningBriefing.desc',
  defaultDesc:
    'Hacker News의 테크 트렌드와 Yahoo Finance 실시간 지표를 결합해 매일 아침 브리핑합니다.',
  badge: 'YOLO',
  executionMode: 'yolo',
  requiredMcpPresets: [{ presetName: 'hn' }, { presetName: 'yahoo-finance' }],
  assistantTemplate: {
    name: '모닝 브리핑 비서',
    description: 'Hacker News 테크 트렌드 및 글로벌 금융 지표 요약 전문 비서',
    systemPrompt:
      '당신은 매일 아침 글로벌 테크 트렌드와 주요 금융 지표를 신속하고 명쾌하게 정리해 주는 전문 브리핑 비서입니다. Hacker News(hn 도구)를 사용해 오늘의 주요 개발/기술 이슈 3가지를 요약하고, Yahoo Finance(yahoo-finance 도구)를 사용해 S&P 500, 나스닥, 주요 환율 등 주요 지표를 함께 엮어 한국어 마크다운 리포트로 작성하세요.',
    allowedBuiltInServiceAliases: ['browser', 'workspace'],
  },
  scheduledTaskTemplate: {
    name: '매일 아침 9시 모닝 브리핑',
    cronExpression: '0 9 * * *',
    executionMode: 'yolo',
    message:
      '오늘의 Hacker News 주요 인기 글 3개와 S&P 500, 나스닥 등 핵심 금융 지표를 종합하여 오늘의 모닝 브리핑 리포트를 작성해줘.',
  },
  testRunPrompt:
    '오늘자 Hacker News 상위 인기 글 3개와 주요 주가지수를 확인하고 지금 바로 모닝 브리핑을 작성해줘.',
};
