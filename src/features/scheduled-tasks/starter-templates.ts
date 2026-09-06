import type { ExecutionMode } from '@/context/agent-session/types';

export interface StarterTaskTemplate {
  id: string;
  titleKey: string;
  defaultTitle: string;
  descKey: string;
  defaultDesc: string;
  name: string;
  cronExpression: string;
  executionMode: ExecutionMode;
  message: string;
  resetPlanningState: boolean;
  preferredAssistantName: string;
}

export const STARTER_TASK_TEMPLATES: StarterTaskTemplate[] = [
  {
    id: 'pc-health-audit',
    titleKey: 'scheduledTasks.starterTemplates.pcAuditTitle',
    defaultTitle: '내 PC 헬스 & 보안 일일 점검',
    descKey: 'scheduledTasks.starterTemplates.pcAuditDesc',
    defaultDesc:
      '매일 자정 디스크 용량, 미커밋 Git 파일, 의심 프로세스 및 패키지 취약점(audit)을 종합 점검합니다.',
    name: '내 PC 헬스 & 보안 일일 점검',
    cronExpression: '0 0 * * *',
    executionMode: 'unsafe',
    message:
      'workspace 쉘을 사용해 디스크 잔여 용량(df -h), 장시간 실행 중인 프로세스, 워크스페이스 내 미커밋 Git 상태 및 패키지 취약점(audit)을 종합 점검하여 일일 브리핑 마크다운 리포트를 작성해줘.',
    resetPlanningState: true,
    preferredAssistantName: 'Coding Expert',
  },
  {
    id: 'web-headline-summary',
    titleKey: 'scheduledTasks.starterTemplates.webSummaryTitle',
    defaultTitle: '웹사이트 변경 및 헤드라인 브리핑',
    descKey: 'scheduledTasks.starterTemplates.webSummaryDesc',
    defaultDesc:
      '매일 아침 브라우저로 기술 뉴스/블로그를 방문해 핵심 헤드라인 3가지를 요약합니다.',
    name: '웹사이트 변경 및 헤드라인 브리핑',
    cronExpression: '0 9 * * *',
    executionMode: 'yolo',
    message:
      '지정된 기술 뉴스/블로그 URL을 browser 도구로 방문하여 오늘의 주요 헤드라인 3가지를 한국어로 3줄 요약해줘.',
    resetPlanningState: true,
    preferredAssistantName: 'Libr Assistant',
  },
];
