import type { AgentSession } from '@/models/agent';

interface StatusBadgeConfig {
  label: string;
  className: string;
}

export function getStatusBadgeConfig(
  status: AgentSession['status'],
): StatusBadgeConfig {
  switch (status) {
    case 'busy':
      return {
        label: 'Active',
        className: 'border-warning/30 bg-warning/10 text-warning-foreground',
      };
    case 'paused':
      return {
        label: 'Paused',
        className:
          'border-muted-foreground/20 bg-muted text-muted-foreground opacity-90',
      };
    case 'error':
      return {
        label: 'Error',
        className:
          'border-destructive/30 bg-destructive/10 text-destructive dark:text-destructive',
      };
    case 'idle':
    default:
      return {
        label: 'Idle',
        className: 'border-border bg-secondary text-secondary-foreground',
      };
  }
}

export function getStatusDotClass(status: AgentSession['status']): string {
  switch (status) {
    case 'busy':
      return 'bg-warning';
    case 'paused':
      return 'bg-muted-foreground/60';
    case 'error':
      return 'bg-destructive';
    case 'idle':
    default:
      return 'bg-emerald-500/80';
  }
}

export function getStatusNodeClass(status: AgentSession['status']): string {
  switch (status) {
    case 'busy':
      return 'border-warning/25 bg-warning/10';
    case 'paused':
      return 'border-muted-foreground/20 bg-muted/60';
    case 'error':
      return 'border-destructive/25 bg-destructive/10';
    case 'idle':
    default:
      return 'border-border/70 bg-background/95';
  }
}
