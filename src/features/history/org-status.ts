interface StatusBadgeConfig {
  label: string;
  className: string;
}

export function getStatusBadgeConfig(status: string): StatusBadgeConfig {
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
