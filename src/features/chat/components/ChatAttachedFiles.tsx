import React from 'react';
import { Button } from '@/components/ui';
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import { AttachmentReference } from '@/models/chat';

export function ChatAttachedFiles() {
  const { pendingFiles, removeFile } = useResourceAttachment();

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
      <div className="text-xs mb-2">📎 Attached Files:</div>
      <div className="flex flex-wrap gap-2">
        {attachedFiles.map((file) => (
          <div
            key={file.contentId}
            className="flex items-center px-2 py-1 rounded border border-gray-700"
          >
            <span className="text-xs truncate max-w-[150px]">
              {file.filename}
            </span>
            <Button
              type="button"
              onClick={() => removeAttachedFile(file)}
              className="ml-2 text-xs"
            >
              ✕
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
