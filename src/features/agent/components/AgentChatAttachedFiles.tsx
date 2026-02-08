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
    <div className="px-4 py-2 border-t">
      <div className="text-xs mb-2 flex items-center gap-1">
        <Paperclip className="w-4 h-4" />
        <span>Attached Files:</span>
      </div>
      <ul className="flex flex-wrap gap-2" aria-label="Attached files">
        {attachedFiles.map((file) => (
          <li
            key={file.contentId}
            className="flex items-center px-2 py-1 rounded-md border border-border bg-muted/20"
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
