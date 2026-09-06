import { useState, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle2,
  AlertCircle,
  Settings,
  Sparkles,
  Calendar,
  Bot,
  Loader2,
} from 'lucide-react';
import { createId } from '@paralleldrive/cuid2';
import { toast } from 'sonner';

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { useSettings } from '@/hooks/use-settings';
import { listConfiguredProviderGroups } from '@/lib/ai-service/configured-providers';
import { buildServerEntityFromPreset } from '@/features/mcp-servers/utils/preset-utils';
import {
  listMCPServerPresets,
  type MCPServerPreset,
} from '@/lib/backend/mcp-server-config';
import { createAssistant, updateAssistant } from '@/lib/backend/assistants';
import {
  createScheduledTask,
  updateScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import { MORNING_BRIEFING_RECIPE } from '../builtin-recipes';
import { useMCPServerActions } from '../hooks/useMCPServerActions';

const logger = getLogger('MorningBriefingWalkthroughDialog');

const FALLBACK_PRESETS: Record<string, MCPServerPreset> = {
  hn: {
    name: 'hn',
    category: 'search',
    description:
      'Read Hacker News — top, new, best, Ask HN, Show HN, and job stories.',
    transportType: 'stdio',
    command: 'npx',
    args: ['-y', '@fre4x/hn'],
  },
  'yahoo-finance': {
    name: 'yahoo-finance',
    category: 'data',
    description:
      'Get real-time stock prices, financials, options chains, and insider data. No API key required.',
    transportType: 'stdio',
    command: 'npx',
    args: ['-y', '@fre4x/yahoo-finance'],
  },
};

export interface MorningBriefingWalkthroughDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function MorningBriefingWalkthroughDialog({
  open,
  onOpenChange,
}: MorningBriefingWalkthroughDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { value: settings } = useSettings();
  const { saveServer } = useMCPServerActions();

  const [selectedPresets, setSelectedPresets] = useState<
    Record<string, boolean>
  >({
    hn: true,
    'yahoo-finance': true,
  });
  const [isSettingUp, setIsSettingUp] = useState(false);

  const hasConfiguredProviders = useMemo(
    () => listConfiguredProviderGroups(settings).length > 0,
    [settings],
  );

  const handleTogglePreset = (presetName: string) => {
    setSelectedPresets((prev) => ({
      ...prev,
      [presetName]: !prev[presetName],
    }));
  };

  const handleSetup = async () => {
    if (!hasConfiguredProviders) {
      toast.error(
        t('recipes.morningBriefing.llmRequiredToast', {
          defaultValue: 'AI 모델 제공자를 먼저 설정해야 합니다.',
        }),
      );
      onOpenChange(false);
      navigate('/settings?tab=ai-models');
      return;
    }

    setIsSettingUp(true);
    try {
      let presets: MCPServerPreset[] = [];
      try {
        presets = await listMCPServerPresets();
      } catch (err) {
        logger.warn('Failed to fetch presets via IPC, using fallback', err);
      }

      // Query existing servers to reuse IDs and avoid duplicate installations
      const existingServers =
        (await safeInvoke<Array<{ id: string; name: string }>>(
          'list_mcp_server_configs',
        )) ?? [];

      const installedServerIds: string[] = [];
      for (const item of MORNING_BRIEFING_RECIPE.requiredMcpPresets) {
        if (!selectedPresets[item.presetName]) continue;

        const existingServer = existingServers.find(
          (s) => s.name === item.presetName,
        );
        if (existingServer?.id) {
          installedServerIds.push(existingServer.id);
          continue;
        }

        const preset =
          presets.find((p) => p.name === item.presetName) ||
          FALLBACK_PRESETS[item.presetName];

        if (preset) {
          const entity = buildServerEntityFromPreset(preset, createId());
          const saved = await saveServer(entity);
          if (saved?.id) {
            installedServerIds.push(saved.id);
          }
        }
      }

      // Query existing assistants to prevent duplicate creation
      const existingAssistants =
        (await safeInvoke<Array<{ id: string; name: string }>>(
          'list_assistants',
        )) ?? [];
      const existingAssistant = existingAssistants.find(
        (a) => a.name === MORNING_BRIEFING_RECIPE.assistantTemplate.name,
      );

      let targetAssistant: { id: string };
      if (existingAssistant) {
        targetAssistant = existingAssistant;
        await updateAssistant({
          id: existingAssistant.id,
          name: MORNING_BRIEFING_RECIPE.assistantTemplate.name,
          description: MORNING_BRIEFING_RECIPE.assistantTemplate.description,
          systemPrompt: MORNING_BRIEFING_RECIPE.assistantTemplate.systemPrompt,
          allowedBuiltInServiceAliases:
            MORNING_BRIEFING_RECIPE.assistantTemplate
              .allowedBuiltInServiceAliases,
          mcpServerIds: installedServerIds,
          deletionProtected: false,
          createdAt: new Date(),
          updatedAt: new Date(),
        });
      } else {
        targetAssistant = await createAssistant({
          id: createId(),
          name: MORNING_BRIEFING_RECIPE.assistantTemplate.name,
          description: MORNING_BRIEFING_RECIPE.assistantTemplate.description,
          systemPrompt: MORNING_BRIEFING_RECIPE.assistantTemplate.systemPrompt,
          allowedBuiltInServiceAliases:
            MORNING_BRIEFING_RECIPE.assistantTemplate
              .allowedBuiltInServiceAliases,
          mcpServerIds: installedServerIds,
          deletionProtected: false,
          createdAt: new Date(),
          updatedAt: new Date(),
        });
      }

      // Query existing tasks to prevent duplicate creation
      const existingTasks =
        (await safeInvoke<Array<{ id: string; name: string }>>(
          'list_scheduled_tasks',
        )) ?? [];
      const existingTask = existingTasks.find(
        (t) => t.name === MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
      );

      if (existingTask) {
        await updateScheduledTask(existingTask.id, {
          name: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
          cronExpression:
            MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.cronExpression,
          executionMode:
            MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.executionMode,
          assistantId: targetAssistant.id,
          message: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.message,
        });
      } else {
        await createScheduledTask({
          name: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
          cronExpression:
            MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.cronExpression,
          executionMode:
            MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.executionMode,
          assistantId: targetAssistant.id,
          message: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.message,
        });
      }

      localStorage.setItem('libragent:morning-briefing:completed', 'true');

      toast.success(
        t('recipes.morningBriefing.setupSuccess', {
          defaultValue: '모닝 브리핑 비서와 예약 작업이 설정되었습니다!',
        }),
      );
      onOpenChange(false);

      // Navigate to draft view with prompt ready and autoSubmit=true
      const promptQuery = encodeURIComponent(
        MORNING_BRIEFING_RECIPE.testRunPrompt,
      );
      navigate(
        `/agent/draft?assistantId=${targetAssistant.id}&prompt=${promptQuery}&autoSubmit=true`,
      );
    } catch (error) {
      logger.error('Failed to setup morning briefing recipe', error);
      toast.error(
        t('recipes.morningBriefing.setupError', {
          defaultValue: '설정 중 오류가 발생했습니다.',
        }),
      );
    } finally {
      setIsSettingUp(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <DialogTitle className="text-xl font-bold">
              {t('recipes.morningBriefing.modalTitle', {
                defaultValue:
                  '🌅 모닝 테크 & 금융 브리핑 세팅 (Morning Briefing Setup)',
              })}
            </DialogTitle>
            <Badge variant="default" className="text-xs bg-primary/80">
              {MORNING_BRIEFING_RECIPE.badge}
            </Badge>
          </div>
          <DialogDescription className="text-sm text-muted-foreground pt-1">
            {t(
              MORNING_BRIEFING_RECIPE.descKey,
              MORNING_BRIEFING_RECIPE.defaultDesc,
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-2">
          {/* Section 1: LLM Check */}
          <div className="rounded-lg border p-4 bg-card space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {t('recipes.morningBriefing.step1Title', {
                  defaultValue: 'Step 1 · AI 모델 설정 확인',
                })}
              </span>
              {hasConfiguredProviders ? (
                <Badge
                  variant="outline"
                  className="gap-1 border-green-600/30 text-green-600 dark:text-green-400 bg-green-500/10"
                >
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  {t('recipes.morningBriefing.connected', {
                    defaultValue: '연동 완료',
                  })}
                </Badge>
              ) : (
                <Badge
                  variant="outline"
                  className="gap-1 border-amber-600/30 text-amber-600 dark:text-amber-400 bg-amber-500/10"
                >
                  <AlertCircle className="h-3.5 w-3.5" />
                  {t('recipes.morningBriefing.required', {
                    defaultValue: '설정 필요',
                  })}
                </Badge>
              )}
            </div>

            {hasConfiguredProviders ? (
              <p className="text-xs text-muted-foreground">
                {t('recipes.morningBriefing.connectedDesc', {
                  defaultValue:
                    'AI 모델 제공자가 연동되어 브리핑 생성 준비가 완료되었습니다.',
                })}
              </p>
            ) : (
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 rounded-md bg-amber-500/10 border border-amber-500/20 p-3">
                <div className="flex items-start gap-2.5">
                  <AlertCircle className="h-4 w-4 text-amber-500 shrink-0 mt-0.5" />
                  <div className="space-y-0.5">
                    <p className="text-xs font-medium text-foreground">
                      {t('recipes.morningBriefing.unconfiguredTitle', {
                        defaultValue: 'AI 모델 제공자가 설정되지 않았습니다.',
                      })}
                    </p>
                    <p className="text-[11px] text-muted-foreground">
                      {t('recipes.morningBriefing.unconfiguredDesc', {
                        defaultValue:
                          '브리핑을 생성하려면 먼저 AI 모델 제공자(OpenAI, Anthropic, Ollama 등)를 설정해야 합니다.',
                      })}
                    </p>
                  </div>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  className="shrink-0 gap-1 text-xs h-7"
                  onClick={() => {
                    onOpenChange(false);
                    navigate('/settings?tab=ai-models');
                  }}
                >
                  <Settings className="h-3.5 w-3.5" />
                  {t('recipes.morningBriefing.goToSettings', {
                    defaultValue: '설정으로 이동',
                  })}
                </Button>
              </div>
            )}
          </div>

          {/* Section 2: Free Extensions */}
          <div className="rounded-lg border p-4 bg-card space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {t('recipes.morningBriefing.step2Title', {
                  defaultValue: 'Step 2 · 무료 확장 도구 설치',
                })}
              </span>
              <span className="text-[11px] text-muted-foreground font-medium">
                {t('recipes.morningBriefing.freeTag', {
                  defaultValue: '💡 100% 무료 · API 키 불필요',
                })}
              </span>
            </div>

            <div className="space-y-2.5">
              <label
                htmlFor="mcp-hn"
                className="flex items-start gap-3 p-2.5 rounded-md border bg-background/50 hover:bg-muted/50 cursor-pointer transition-colors"
              >
                <Checkbox
                  id="mcp-hn"
                  checked={!!selectedPresets['hn']}
                  onCheckedChange={() => handleTogglePreset('hn')}
                  className="mt-0.5"
                />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-foreground">
                      {t('recipes.morningBriefing.hnTitle', {
                        defaultValue: 'Hacker News (hn)',
                      })}
                    </span>
                    <Badge variant="secondary" className="text-[10px] h-4 px-1">
                      search
                    </Badge>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {t('recipes.morningBriefing.hnDesc', {
                      defaultValue:
                        'Hacker News 실시간 인기/신규 개발 및 기술 트렌드 기사 조회',
                    })}
                  </p>
                </div>
              </label>

              <label
                htmlFor="mcp-yahoo-finance"
                className="flex items-start gap-3 p-2.5 rounded-md border bg-background/50 hover:bg-muted/50 cursor-pointer transition-colors"
              >
                <Checkbox
                  id="mcp-yahoo-finance"
                  checked={!!selectedPresets['yahoo-finance']}
                  onCheckedChange={() => handleTogglePreset('yahoo-finance')}
                  className="mt-0.5"
                />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-foreground">
                      {t('recipes.morningBriefing.yfTitle', {
                        defaultValue: 'Yahoo Finance (yahoo-finance)',
                      })}
                    </span>
                    <Badge variant="secondary" className="text-[10px] h-4 px-1">
                      data
                    </Badge>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {t('recipes.morningBriefing.yfDesc', {
                      defaultValue:
                        'S&P 500, 나스닥, 환율, 개별 종목 등 실시간 글로벌 금융 지표 조회 (API 키 없이 즉시 사용)',
                    })}
                  </p>
                </div>
              </label>
            </div>
          </div>

          {/* Section 3: Assistant & Schedule Preview */}
          <div className="rounded-lg border p-4 bg-card space-y-3">
            <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t('recipes.morningBriefing.step3Title', {
                defaultValue: 'Step 3 · 비서 및 자동 실행 스케줄',
              })}
            </span>

            <div className="space-y-2">
              <div className="flex items-start gap-2.5 p-2.5 rounded-md bg-muted/40 border">
                <Bot className="h-4 w-4 text-primary shrink-0 mt-0.5" />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-1.5">
                    <p className="text-xs font-medium text-foreground">
                      {MORNING_BRIEFING_RECIPE.assistantTemplate.name}
                    </p>
                    <Badge variant="outline" className="text-[10px] h-4 px-1">
                      {t('recipes.morningBriefing.builtinToolsBadge', {
                        defaultValue: '내장 도구 연동',
                      })}
                    </Badge>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {MORNING_BRIEFING_RECIPE.assistantTemplate.description}
                  </p>
                </div>
              </div>

              <div className="flex items-start gap-2.5 p-2.5 rounded-md bg-muted/40 border">
                <Calendar className="h-4 w-4 text-primary shrink-0 mt-0.5" />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-1.5">
                    <p className="text-xs font-medium text-foreground">
                      {MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name}
                    </p>
                    <Badge
                      variant="default"
                      className="text-[10px] h-4 px-1.5 bg-primary/80"
                    >
                      {t('recipes.morningBriefing.scheduleBadge', {
                        defaultValue: '매일 09:00 AM (YOLO 모드)',
                      })}
                    </Badge>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.message}
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <DialogFooter className="flex flex-col sm:flex-row sm:justify-end gap-2 pt-2 border-t">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onOpenChange(false)}
            disabled={isSettingUp}
          >
            {t('common.cancel', { defaultValue: '취소' })}
          </Button>
          <Button
            size="sm"
            className="gap-1.5"
            onClick={handleSetup}
            disabled={isSettingUp || !hasConfiguredProviders}
          >
            {isSettingUp ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                {t('recipes.morningBriefing.settingUp', {
                  defaultValue: '설정 중...',
                })}
              </>
            ) : (
              <>
                <Sparkles className="h-4 w-4" />
                {t('recipes.morningBriefing.actionButton', {
                  defaultValue: '🚀 자동 설정 & 지금 브리핑 받아보기',
                })}
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
