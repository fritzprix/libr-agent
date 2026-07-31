import LoadingSpinner from '@/components/ui/LoadingSpinner';

interface SessionLoadingOverlayProps {
  label: string;
  initializationStep?: string | null;
  initializationError?: string | null;
  variant: 'blocking' | 'overlay' | 'banner';
}

export function SessionLoadingOverlay({
  label,
  initializationStep,
  initializationError,
  variant,
}: SessionLoadingOverlayProps) {
  const isFailed = Boolean(initializationError);

  if (variant === 'banner') {
    return (
      <div
        role="status"
        aria-live="polite"
        className="absolute top-0 left-0 right-0 z-20 flex items-center justify-between border-b border-amber-200/50 bg-amber-50/90 px-4 py-2 text-xs text-amber-800 backdrop-blur-sm dark:border-amber-900/40 dark:bg-amber-950/80 dark:text-amber-200 animate-in fade-in slide-in-from-top-1 duration-200"
      >
        <div className="flex items-center gap-2">
          {!isFailed ? (
            <LoadingSpinner
              size="sm"
              className="border-2 text-amber-600 dark:text-amber-400 shrink-0"
              label={label}
            />
          ) : null}
          <span className="font-medium">
            {initializationError ?? initializationStep ?? label}
          </span>
        </div>
      </div>
    );
  }

  const content = (
    <>
      {!isFailed ? (
        <LoadingSpinner size="lg" className="border-4" label={label} />
      ) : null}

      <div className="flex flex-col items-center gap-1">
        <div
          className={
            isFailed
              ? 'text-destructive font-medium'
              : variant === 'blocking'
                ? 'text-muted-foreground font-medium animate-pulse'
                : 'text-muted-foreground font-medium'
          }
          aria-hidden="true"
        >
          {label}
        </div>

        <div
          className={
            isFailed
              ? 'text-xs text-destructive/80 max-w-sm text-center'
              : 'text-xs text-muted-foreground/70 h-4'
          }
        >
          {isFailed ? (
            <span>{initializationError}</span>
          ) : initializationStep ? (
            <span className="animate-in fade-in slide-in-from-bottom-1 duration-300">
              {initializationStep}
            </span>
          ) : null}
        </div>
      </div>
    </>
  );

  if (variant === 'blocking') {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <div className="flex flex-col items-center gap-3">{content}</div>
      </div>
    );
  }

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/60 backdrop-blur-[1px]">
      <div className="flex flex-col items-center gap-3 rounded-xl border border-border/60 bg-background/90 px-6 py-5 shadow-lg">
        {content}
      </div>
    </div>
  );
}
