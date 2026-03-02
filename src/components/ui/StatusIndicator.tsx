import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';

interface StatusIndicatorProps {
  status: 'connected' | 'disconnected' | 'unknown' | 'connecting';
  label?: string;
  showLabel?: boolean;
  size?: 'sm' | 'md' | 'lg';
}

export default function StatusIndicator({
  status,
  label,
  showLabel = false,
  size = 'md',
}: StatusIndicatorProps) {
  const { t } = useTranslation('common');

  const statusColors = {
    connected: 'bg-success',
    disconnected: 'bg-destructive',
    unknown: 'bg-muted-foreground',
    connecting: 'bg-warning animate-pulse',
  };

  const statusTexts = {
    connected: t('status.connected', 'Connected'),
    disconnected: t('status.disconnected', 'Disconnected'),
    unknown: t('status.unknown', 'Unknown'),
    connecting: t('status.connecting', 'Connecting...'),
  };

  const sizeClasses = {
    sm: 'w-2 h-2',
    md: 'w-3 h-3',
    lg: 'w-4 h-4',
  };

  const displayLabel = label || statusTexts[status];

  return (
    <div className="flex items-center gap-1">
      <div
        className={cn('rounded-full', statusColors[status], sizeClasses[size])}
        title={displayLabel}
      />
      {showLabel && (
        <span className="text-xs text-muted-foreground">{displayLabel}</span>
      )}
    </div>
  );
}
