import { Package } from 'lucide-react';
import { MCPServerEntity } from '@/models/chat';
import type { MCPServerMetadata } from '../hooks/useMCPServerForm';

interface PresetServerSummaryProps {
  server: MCPServerEntity;
}

export function PresetServerSummary({ server }: PresetServerSummaryProps) {
  const metadata = server.metadata as MCPServerMetadata | undefined;
  const description = metadata?.description?.trim();
  const logo = metadata?.logo?.trim();

  return (
    <div className="flex gap-3 items-start rounded-md border bg-muted/10 p-3">
      <div className="h-10 w-10 shrink-0 overflow-hidden rounded-md border bg-background">
        {logo ? (
          <img
            src={logo}
            alt=""
            className="h-full w-full object-contain p-1"
            onError={(event) => {
              event.currentTarget.style.display = 'none';
              const fallback = event.currentTarget.nextElementSibling;
              if (fallback instanceof HTMLElement) {
                fallback.classList.remove('hidden');
              }
            }}
          />
        ) : null}
        <div
          className={`flex h-full w-full items-center justify-center text-muted-foreground ${logo ? 'hidden' : ''}`}
        >
          <Package className="h-5 w-5" aria-hidden />
        </div>
      </div>
      <div className="min-w-0 space-y-1">
        <p className="truncate text-sm font-semibold">{server.name}</p>
        {description ? (
          <p className="text-xs text-muted-foreground leading-relaxed">
            {description}
          </p>
        ) : null}
      </div>
    </div>
  );
}
