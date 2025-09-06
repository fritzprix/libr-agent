import React, { useCallback, useState } from 'react';
import { Button, FileAttachment, Input } from '@/components/ui';
import { Send, Square, Loader2 } from 'lucide-react';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/context/ChatContext';
import { useFileAttachment } from '../hooks/useFileAttachment';
import { getLogger } from '@/lib/logger';
import { AttachmentReference, Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { stringToMCPContentArray } from '@/lib/utils';

const logger = getLogger('ChatInput');

interface ChatInputProps {
  children?: React.ReactNode;
}

export function ChatInput({ children }: ChatInputProps) {
  const [input, setInput] = useState<string>('');

  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const {
    submit,
    isLoading,
    isToolExecuting,
    cancel,
    addToMessageQueue,
    pendingCancel,
  } = useChatContext();
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

      if (isToolExecuting) {
        // Tool 실행 중이면 메시지를 큐에 추가
        addToMessageQueue({
          content: stringToMCPContentArray(input.trim()),
          attachments: attachedFiles,
        });
        setInput('');
        clearPendingFiles();
        logger.info('Message queued during tool execution');
      } else {
        // 일반 전송
        const userMessage: Message = {
          id: createId(),
          content: stringToMCPContentArray(input.trim()),
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
      isToolExecuting,
      addToMessageQueue,
    ],
  );

  const handleCancel = useCallback(() => {
    cancel();
  }, [cancel]);

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
      className={`px-4 py-4 border-t flex items-center gap-2 transition-colors ${dragState === 'valid'
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
          className={`flex-1 min-w-0 transition-colors ${dragState === 'valid'
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

      <div className="flex gap-2">
        <Button
          type="submit"
          disabled={
            isAttachmentLoading || (!input.trim() && attachedFiles.length === 0)
          }
          variant="ghost"
          size="icon"
          title={isToolExecuting ? 'Queue message' : 'Send message'}
        >
          <Send className="h-4 w-4" />
        </Button>

        {isLoading && (
          <Button
            onClick={handleCancel}
            variant="destructive"
            size="icon"
            disabled={pendingCancel}
            title={pendingCancel ? 'Cancelling...' : 'Cancel request'}
          >
            {pendingCancel ? (
              <Loader2 className="h-4 w-4 animate-spin text-amber-500" />
            ) : (
              <Square className="h-4 w-4" />
            )}
          </Button>
        )}
      </div>
    </form>
  );
}
