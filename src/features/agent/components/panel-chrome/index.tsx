import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';
import { AlertTriangle, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';

export function PanelEyebrow({
  icon,
  children,
  className,
}: {
  icon?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground',
        className,
      )}
    >
      {icon}
      <span>{children}</span>
    </div>
  );
}

export function PanelSummaryPill({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground',
        className,
      )}
    >
      {children}
    </div>
  );
}

export function PanelListFrame({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'min-h-0 flex-1 overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]',
        className,
      )}
    >
      {children}
    </div>
  );
}

export function PanelEmptyState({
  icon,
  children,
  className,
}: {
  icon?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center',
        className,
      )}
    >
      {icon}
      <p className="max-w-[16rem] text-sm text-muted-foreground">{children}</p>
    </div>
  );
}

export function PanelErrorState({
  message,
  onRetry,
  retryLabel,
  className,
}: {
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center',
        className,
      )}
    >
      <AlertTriangle
        className="h-5 w-5 text-destructive/80"
        aria-hidden="true"
      />
      <p className="max-w-[16rem] text-sm text-muted-foreground">{message}</p>
      {onRetry ? (
        <Button type="button" variant="outline" size="sm" onClick={onRetry}>
          {retryLabel ?? 'Retry'}
        </Button>
      ) : null}
    </div>
  );
}

export function PanelLoadingState({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-1 flex-col items-center justify-center gap-3 p-6 text-muted-foreground',
        className,
      )}
    >
      <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
      <p className="text-sm">{children}</p>
    </div>
  );
}
