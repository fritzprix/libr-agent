import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';

interface LoadingSpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  label?: string;
}

export default function LoadingSpinner({
  size = 'md',
  className = '',
  label,
}: LoadingSpinnerProps) {
  const { t } = useTranslation('common');
  const sizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-6 h-6',
    lg: 'w-8 h-8',
  };

  const displayLabel = label || t('common.loading', 'Loading...');

  return (
    <div
      role="status"
      aria-label={displayLabel}
      className={cn(
        'animate-spin rounded-full border-2 border-muted border-t-primary',
        sizeClasses[size],
        className,
      )}
    >
      <span className="sr-only">{displayLabel}</span>
    </div>
  );
}
