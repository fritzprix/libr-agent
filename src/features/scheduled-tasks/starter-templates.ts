import type { ExecutionMode } from '@/context/agent-session/types';

export interface StarterTaskTemplate {
  id: string;
  titleKey: string;
  defaultTitle: string;
  descKey: string;
  defaultDesc: string;
  nameKey?: string;
  defaultName?: string;
  name: string;
  cronExpression: string;
  executionMode: ExecutionMode;
  messageKey?: string;
  defaultMessage?: string;
  message: string;
  resetPlanningState: boolean;
  preferredAssistantName: string;
}

const KNOWLEDGE_DISTILL_MESSAGE =
  "Review today's and recent sessions. Save only durable knowledge to Knowledge: architecture decisions, confirmed bug fixes, project rules/preferences, and important APIs or settings. Skip daily news, one-off snapshots, and chit-chat. Follow the knowledge-distiller skill: search Knowledge first to avoid duplicates, record concise stand-alone summaries with a few high-value entities/relationships, tag with distilled, and leave auto_extract off. Do not promote section headers, ISO dates, or short acronyms as entities.";

export const STARTER_TASK_TEMPLATES: StarterTaskTemplate[] = [
  {
    id: 'knowledge-distill',
    titleKey: 'scheduledTasks.starterTemplates.knowledgeDistillTitle',
    defaultTitle: 'Daily durable knowledge distillation',
    descKey: 'scheduledTasks.starterTemplates.knowledgeDistillDesc',
    defaultDesc:
      'Every evening, distill lasting decisions and fixes from recent sessions into Knowledge — not daily news or temporary notes.',
    nameKey: 'scheduledTasks.starterTemplates.knowledgeDistillName',
    defaultName: 'Daily durable knowledge distillation',
    name: 'Daily durable knowledge distillation',
    cronExpression: '0 21 * * *',
    executionMode: 'yolo',
    messageKey: 'scheduledTasks.starterTemplates.knowledgeDistillMessage',
    defaultMessage: KNOWLEDGE_DISTILL_MESSAGE,
    message: KNOWLEDGE_DISTILL_MESSAGE,
    resetPlanningState: true,
    preferredAssistantName: 'Libr Assistant',
  },
  {
    id: 'pc-health-audit',
    titleKey: 'scheduledTasks.starterTemplates.pcAuditTitle',
    defaultTitle: '내 PC 헬스 & 보안 일일 점검',
    descKey: 'scheduledTasks.starterTemplates.pcAuditDesc',
    defaultDesc:
      '매일 자정 디스크 용량, 미커밋 Git 파일, 의심 프로세스 및 패키지 취약점(audit)을 종합 점검합니다.',
    nameKey: 'scheduledTasks.starterTemplates.pcAuditName',
    defaultName: '내 PC 헬스 & 보안 일일 점검',
    name: '내 PC 헬스 & 보안 일일 점검',
    cronExpression: '0 0 * * *',
    executionMode: 'unsafe',
    messageKey: 'scheduledTasks.starterTemplates.pcAuditMessage',
    defaultMessage:
      'workspace 쉘을 사용해 디스크 잔여 용량(df -h), 장시간 실행 중인 프로세스, 워크스페이스 내 미커밋 Git 상태 및 패키지 취약점(audit)을 종합 점검하여 일일 브리핑 마크다운 리포트를 작성해줘.',
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
    nameKey: 'scheduledTasks.starterTemplates.webSummaryName',
    defaultName: '웹사이트 변경 및 헤드라인 브리핑',
    name: '웹사이트 변경 및 헤드라인 브리핑',
    cronExpression: '0 9 * * *',
    executionMode: 'yolo',
    messageKey: 'scheduledTasks.starterTemplates.webSummaryMessage',
    defaultMessage:
      'Hacker News(https://news.ycombinator.com)를 browser 도구로 방문하여 오늘의 주요 기술 헤드라인 3가지를 한국어로 3줄 요약해줘.',
    message:
      'Hacker News(https://news.ycombinator.com)를 browser 도구로 방문하여 오늘의 주요 기술 헤드라인 3가지를 한국어로 3줄 요약해줘.',
    resetPlanningState: true,
    preferredAssistantName: 'Libr Assistant',
  },
];
