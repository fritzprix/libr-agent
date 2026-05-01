import { Paperclip, X } from 'lucide-react';
import { Button } from '@/components/ui';

interface AgentAttachedFileItem {
  id: string;
  name: string;
  onRemove: () => void;
}

interface AgentAttachedFilesBarProps {
  files: AgentAttachedFileItem[];
  title?: string;
}

export function AgentAttachedFilesBar({
  files,
  title = 'Attached Files:',
}: AgentAttachedFilesBarProps) {
  if (files.length === 0) return null;

  return (
    <div className="rounded-t-xl border-x border-t border-border/40 bg-background/40 px-4 py-2 supports-[backdrop-filter]:bg-background/25 backdrop-blur-xl">
      <div className="mb-2 flex items-center gap-1 font-sans text-xs font-medium uppercase tracking-tight text-muted-foreground">
        <Paperclip className="h-4 w-4" />
        <span>{title}</span>
      </div>
      <ul className="flex flex-wrap gap-2" aria-label="Attached files">
        {files.map((file) => (
          <li
            key={file.id}
            className="flex items-center rounded-md border border-border/45 bg-background/45 px-2 py-1"
          >
            <span className="max-w-36 truncate text-xs">{file.name}</span>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={file.onRemove}
              className="ml-1 h-6 w-6"
              title={`Remove ${file.name}`}
              aria-label={`Remove ${file.name}`}
            >
              <X className="h-4 w-4" />
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
