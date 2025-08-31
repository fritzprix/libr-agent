import React, { useCallback, useState } from 'react';
import { Button, FileAttachment, Input } from '@/components/ui';
import { Send, X } from 'lucide-react';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/context/ChatContext';
import { useFileAttachment } from '../hooks/useFileAttachment';
import { getLogger } from '@/lib/logger';
import { AttachmentReference, Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('ChatInput');

interface ChatInputProps {
  children?: React.ReactNode;
}

export function ChatInput({ children }: ChatInputProps) {
  const [input, setInput] = useState<string>('');

  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { submit, isLoading, cancel } = useChatContext();
  const {
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    isAttachmentLoading,
    dragState,
    handleFileAttachment,
    removeFile,
  } = useFileAttachment();

  const attachedFiles = pendingFiles;

  const handleAgentInputChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setInput(e.target.value);
    },
    [setInput],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();

      if (isLoading) {
        cancel();
        return;
      }

      if (!input.trim() && pendingFiles.length === 0) return;
      if (!currentAssistant || !currentSession) return;

      let attachedFiles: AttachmentReference[] = [];

      if (pendingFiles.length > 0) {
        try {
          attachedFiles = await commitPendingFiles();
        } catch (err) {
          logger.error('Error uploading pending files:', err);
          alert('파일 업로드 중 오류가 발생했습니다.');
          return;
        }
      }

      const userMessage: Message = {
        id: createId(),
        content: input.trim(),
        role: 'user',
        sessionId: currentSession.id,
        attachments: attachedFiles,
      };

      setInput('');
      clearPendingFiles();

      try {
        await submit([userMessage]);
      } catch (err) {
        logger.error('Error submitting message:', err);
      }
    },
    [
      submit,
      input,
      pendingFiles,
      currentAssistant,
      currentSession,
      commitPendingFiles,
      clearPendingFiles,
      isLoading,
      cancel,
    ],
  );

  const removeAttachedFile = React.useCallback(
    (filename: string) => {
      const fileToRemove = attachedFiles.find(
        (f: AttachmentReference) => f.filename === filename,
      );
      if (fileToRemove) {
        removeFile(fileToRemove);
      }
    },
    [attachedFiles, removeFile],
  );

  return (
    <form
      onSubmit={handleSubmit}
      className={`px-4 py-4 border-t flex items-center gap-2 transition-colors ${
        dragState === 'valid'
          ? 'bg-green-500/10 border-green-500'
          : dragState === 'invalid'
            ? 'bg-destructive/10 border-destructive'
            : ''
      }`}
    >
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <Input
          value={input}
          onChange={handleAgentInputChange}
          placeholder={
            dragState !== 'none'
              ? dragState === 'valid'
                ? 'Drop supported files here...'
                : 'Unsupported file type!'
              : isLoading || isAttachmentLoading
                ? 'Agent busy...'
                : 'Query agent or drop files...'
          }
          disabled={isLoading || isAttachmentLoading}
          className={`flex-1 min-w-0 transition-colors ${
            dragState === 'valid'
              ? 'border-green-500 bg-green-500/10'
              : dragState === 'invalid'
                ? 'border-destructive bg-destructive/10'
                : ''
          }`}
          autoComplete="off"
          spellCheck="false"
        />

        <FileAttachment
          files={attachedFiles.map((file: AttachmentReference) => ({
            name: file.filename,
            content: file.preview || '',
          }))}
          onRemove={(index: number) => {
            const file = attachedFiles[index];
            if (file) {
              removeAttachedFile(file.filename);
            }
          }}
          onAdd={handleFileAttachment}
          compact={true}
        />
        {children}
      </div>

      <Button
        type="submit"
        disabled={
          isAttachmentLoading ||
          (!isLoading && !input.trim() && attachedFiles.length === 0)
        }
        variant="ghost"
        size="icon"
        title={isLoading ? 'Cancel request' : 'Send message'}
      >
        {isLoading ? <X className="h-4 w-4" /> : <Send className="h-4 w-4" />}
      </Button>
    </form>
  );
}
