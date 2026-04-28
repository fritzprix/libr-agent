import React from 'react';
import { Paperclip, X } from 'lucide-react';
import { Button } from '@/components/ui';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { AttachmentReference } from '@/models/chat';

export function AgentChatAttachedFiles() {
  const { pendingFiles, removeFile } = useAgentResourceAttachment();

  const attachedFiles = pendingFiles;

  const removeAttachedFile = React.useCallback(
    (file: AttachmentReference) => {
      removeFile(file);
    },
    [removeFile],
  );

  if (attachedFiles.length === 0) return null;

  return (
    <div className="rounded-t-xl border-x border-t border-border/40 bg-background/40 px-4 py-2 supports-[backdrop-filter]:bg-background/25 backdrop-blur-xl">
      <div className="text-xs mb-2 flex items-center gap-1 font-medium text-muted-foreground font-sans uppercase tracking-tight">
        <Paperclip className="w-4 h-4" />
        <span>Attached Files:</span>
      </div>
      <ul className="flex flex-wrap gap-2" aria-label="Attached files">
        {attachedFiles.map((file) => (
          <li
            key={file.contentId}
            className="flex items-center rounded-md border border-border/45 bg-background/45 px-2 py-1"
          >
            <span className="text-xs truncate max-w-36">{file.filename}</span>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => removeAttachedFile(file)}
              className="ml-1 h-6 w-6"
              title={`Remove ${file.filename}`}
              aria-label={`Remove ${file.filename}`}
            >
              <X className="w-4 h-4" />
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
