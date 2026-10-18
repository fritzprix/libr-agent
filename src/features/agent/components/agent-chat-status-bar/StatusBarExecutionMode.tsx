import { Button } from '@/components/ui/button';
import type { ExecutionMode } from '@/context/agent-session/types';
import { cn } from '@/lib/utils';
import { DatabaseZap, Shield, Zap } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

export interface StatusBarExecutionModeProps {
  executionMode: ExecutionMode;
  onExecutionModeChange: (mode: ExecutionMode) => void;
}

export function StatusBarExecutionMode({
  executionMode,
  onExecutionModeChange,
}: StatusBarExecutionModeProps) {
  const { t } = useTranslation();

  const executionModeOptions = useMemo(
    () => [
      {
        mode: 'normal' as const,
        label: 'Normal',
        icon: Shield,
        title: t(
          'agent.statusBar.executionModeNormalTitle',
          'Normal mode. Policy blocks and approval rules are enforced.',
        ),
        activeClass: 'bg-secondary text-foreground',
        iconClass: 'text-foreground',
      },
      {
        mode: 'yolo' as const,
        label: 'YOLO',
        icon: Zap,
        title: t(
          'agent.statusBar.executionModeYoloTitle',
          'YOLO mode. Standard approvals are auto-approved, but hard approvals still require manual approval.',
        ),
        activeClass: 'bg-primary/10 text-primary',
        iconClass: 'fill-primary text-primary',
      },
      {
        mode: 'unsafe' as const,
        label: 'Full Auto',
        icon: DatabaseZap,
        title: t(
          'agent.statusBar.executionModeUnsafeTitle',
          'Full Auto mode. All tool executions are automatically approved for uninterrupted workflow.',
        ),
        activeClass: 'bg-indigo-500/10 text-indigo-600 dark:text-indigo-400',
        iconClass: 'fill-indigo-500 text-indigo-600 dark:text-indigo-400',
      },
    ],
    [t],
  );

  return (
    <div
      className="flex flex-wrap items-center gap-1.5 sm:gap-2"
      data-testid="execution-mode-control"
    >
      <span className="shrink-0 text-[11px] text-muted-foreground sm:text-xs">
        Execution
      </span>
      <div className="inline-flex shrink-0 items-center rounded-md border border-border/70 bg-background/60 p-0.5">
        {executionModeOptions.map((option) => {
          const Icon = option.icon;
          const isActive = executionMode === option.mode;

          return (
            <Button
              key={option.mode}
              variant="ghost"
              size="sm"
              onClick={() => onExecutionModeChange(option.mode)}
              className={cn(
                'flex h-6 items-center gap-1 rounded-sm px-1.5 text-[11px] sm:px-2 sm:text-xs',
                isActive
                  ? option.activeClass
                  : 'text-muted-foreground hover:bg-muted',
              )}
              title={option.title}
            >
              <Icon
                size={14}
                className={isActive ? option.iconClass : 'text-current'}
              />
              {option.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
