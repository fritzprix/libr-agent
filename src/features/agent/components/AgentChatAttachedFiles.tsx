import React from 'react';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { AttachmentReference } from '@/models/chat';
import { AgentAttachedFilesBar } from './AgentAttachedFilesBar';

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
    <AgentAttachedFilesBar
      files={attachedFiles.map((file) => ({
        id: file.contentId ?? file.filename,
        name: file.filename,
        onRemove: () => removeAttachedFile(file),
      }))}
    />
  );
}
