import LoadingSpinner from '@/components/ui/LoadingSpinner';

interface SessionLoadingOverlayProps {
  label: string;
  initializationStep?: string | null;
  variant: 'blocking' | 'overlay';
}

export function SessionLoadingOverlay({
  label,
  initializationStep,
  variant,
}: SessionLoadingOverlayProps) {
  const content = (
    <>
      <LoadingSpinner size="lg" className="border-4" label={label} />

      <div className="flex flex-col items-center gap-1">
        <div
          className={
            variant === 'blocking'
              ? 'text-muted-foreground font-medium animate-pulse'
              : 'text-muted-foreground font-medium'
          }
          aria-hidden="true"
        >
          {label}
        </div>

        <div className="text-xs text-muted-foreground/70 h-4">
          {initializationStep ? (
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
