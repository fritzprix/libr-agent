import { memo, useEffect, useState } from 'react';
import { cn } from '@/lib/utils';
import { renderMermaidSvg } from '@/lib/mermaid/loader';

export interface MermaidBlockProps {
  code: string;
  isDark?: boolean;
  className?: string;
}

function MermaidSkeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        'animate-pulse rounded-lg border border-border bg-muted/30 p-6 min-h-[8rem]',
        className,
      )}
      data-testid="mermaid-skeleton"
      aria-busy="true"
      aria-label="Rendering diagram"
    >
      <div className="h-3 w-1/3 rounded bg-muted mb-3" />
      <div className="h-24 w-full rounded bg-muted/80" />
    </div>
  );
}

/**
 * Renders a ```mermaid fence as SVG via lazy-loaded mermaid.
 * Incomplete/invalid syntax shows an error with expandable source (streaming-safe).
 */
export const MermaidBlock = memo(
  ({ code, isDark = false, className }: MermaidBlockProps) => {
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [showSource, setShowSource] = useState(false);

    useEffect(() => {
      let cancelled = false;
      setSvg(null);
      setError(null);

      // Debounce: streaming may close the fence early with partial source.
      const timer = window.setTimeout(() => {
        void renderMermaidSvg(code, isDark)
          .then((rendered) => {
            if (!cancelled) {
              setSvg(rendered);
              setError(null);
            }
          })
          .catch((err: unknown) => {
            if (!cancelled) {
              const message =
                err instanceof Error ? err.message : String(err);
              setError(message);
              setSvg(null);
            }
          });
      }, 120);

      return () => {
        cancelled = true;
        window.clearTimeout(timer);
      };
    }, [code, isDark]);

    if (error) {
      return (
        <div
          className={cn(
            'rounded-lg border border-destructive/40 bg-destructive/5 p-3 text-sm',
            className,
          )}
          data-testid="mermaid-error"
        >
          <p className="text-destructive font-medium mb-2">
            Failed to render Mermaid diagram
          </p>
          <p className="text-muted-foreground text-xs mb-2 break-words">
            {error}
          </p>
          <button
            type="button"
            className="text-xs underline text-foreground/80 hover:text-foreground"
            onClick={() => setShowSource((prev) => !prev)}
          >
            {showSource ? 'Hide source' : 'Show source'}
          </button>
          {showSource ? (
            <pre className="mt-2 overflow-x-auto rounded-md bg-muted/50 p-2 font-mono text-xs whitespace-pre">
              {code}
            </pre>
          ) : null}
        </div>
      );
    }

    if (!svg) {
      return <MermaidSkeleton className={className} />;
    }

    return (
      <div
        className={cn(
          'mermaid-block overflow-x-auto rounded-lg border border-border bg-muted/30 p-4',
          className,
        )}
        data-testid="mermaid-diagram"
        // mermaid.securityLevel=strict returns sanitized SVG
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    );
  },
);

MermaidBlock.displayName = 'MermaidBlock';
