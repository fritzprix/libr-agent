import React, {
  useCallback,
  useState,
  useRef,
  useEffect,
  useMemo,
} from 'react';
import { Button, FileAttachment } from '@/components/ui';
import { Send, Square, Loader2 } from 'lucide-react';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatState, useChatActions } from '@/context/ChatContext';
import { useFileAttachment } from '../hooks/useFileAttachment';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { AttachmentReference } from '@/models/chat';
import { stringToMCPContentArray } from '@/lib/utils';
import { createUserMessage } from '@/lib/chat-utils';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';

const logger = getLogger('ChatInput');

const textareaStyle = {
  msOverflowStyle: 'none',
  scrollbarWidth: 'none',
} as const;

interface ChatInputProps {
  children?: React.ReactNode;
}

export function ChatInput({ children }: ChatInputProps) {
  const [input, setInput] = useState<string>('');
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const chatInputRef = useRef<HTMLFormElement>(null);
  const { subscribe } = useDnDContext();

  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { isLoading, isToolExecuting, pendingCancel } = useChatState();
  const { submit, cancel, addToMessageQueue } = useChatActions();
  const {
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    isAttachmentLoading,
    handleFileAttachment,
    removeFile,
    processFileDrop,
    validateFiles,
  } = useFileAttachment();

  const attachedFiles = pendingFiles;

  const inputPlaceholder = useMemo(() => {
    if (dragState !== 'none') {
      return dragState === 'valid'
        ? 'Drop supported files here...'
        : 'Unsupported file type!';
    }
    if (isLoading || isAttachmentLoading) return 'Agent busy...';
    return 'Query agent or drop files...';
  }, [dragState, isLoading, isAttachmentLoading]);

  const inputClassName = useMemo(() => {
    return `flex-1 min-w-0 transition-colors ${
      dragState === 'valid'
        ? 'border-green-500 bg-green-500/10'
        : dragState === 'invalid'
          ? 'border-destructive bg-destructive/10'
          : ''
    }`;
  }, [dragState]);

  const formClassName = useMemo(() => {
    return `px-4 py-4 border-t flex items-center gap-2 transition-colors ${
      dragState === 'valid'
        ? 'bg-green-500/10 border-green-500'
        : dragState === 'invalid'
          ? 'bg-destructive/10 border-destructive'
          : ''
    }`;
  }, [dragState]);

  const handleAgentInputChange = React.useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setInput(e.target.value);
    },
    [],
  );

  // Handle Enter/Shift+Enter for line breaks and submission
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (
          !isLoading &&
          !isAttachmentLoading &&
          (input.trim() || attachedFiles.length > 0)
        ) {
          chatInputRef.current?.dispatchEvent(
            new Event('submit', { bubbles: true, cancelable: true }),
          );
        }
      }
    },
    [isLoading, isAttachmentLoading, input, attachedFiles.length],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();

      if (!input.trim() && pendingFiles.length === 0) {
        logger.info('Submit ignored: no input and no pending files');
        return;
      }
      if (!currentAssistant || !currentSession) return;

      let attachedFiles: AttachmentReference[] = [];

      if (pendingFiles.length > 0) {
        try {
          logger.info('About to commit pending files', {
            pendingCount: pendingFiles.length,
            filenames: pendingFiles.map((f) => f.filename),
          });
          attachedFiles = await commitPendingFiles();
          logger.info('Pending files committed', {
            attachedCount: attachedFiles.length,
          });
        } catch (err) {
          logger.error('Error uploading pending files:', err);
          toast.error('파일 업로드에 실패했습니다');
          return;
        }
      }

      if (isToolExecuting) {
        // If a tool is running, add the message to the queue
        addToMessageQueue({
          content: stringToMCPContentArray(input.trim()),
          attachments: attachedFiles,
        });
        setInput('');
        clearPendingFiles();
        logger.info('Message queued during tool execution');
      } else {
        // Normal submission
        const userMessage = createUserMessage(input.trim(), currentSession.id);

        // Add attachments if they exist
        if (attachedFiles.length > 0) {
          userMessage.attachments = attachedFiles;
        }

        try {
          logger.info('Submitting user message', {
            hasAttachments: attachedFiles.length > 0,
            attachmentCount: attachedFiles.length,
          });
          await submit([userMessage]);
          logger.info('User message submitted successfully');
          // Clear input and files only on success
          setInput('');
          clearPendingFiles();
        } catch (err) {
          logger.error('Error submitting message:', err);
          // Keep input value on failure
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

  useEffect(() => {
    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      if (event === 'drag-over') {
        logger.info('Drag Over ', { event, payload });
        const isValid = payload.paths ? validateFiles(payload.paths) : false;
        setDragState(isValid ? 'valid' : 'invalid');
      } else if (event === 'drop') {
        setDragState('none');
        if (payload.paths) {
          processFileDrop(payload.paths);
        }
      } else if (event === 'leave') {
        setDragState('none');
      }
    };

    const unsub = subscribe(chatInputRef, handler, { priority: 10 });
    return () => unsub();
  }, [subscribe, processFileDrop, validateFiles]);

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

  const fileAttachmentFiles = useMemo(
    () =>
      attachedFiles.map((file: AttachmentReference) => ({
        name: file.filename,
        content: file.preview || '',
      })),
    [attachedFiles],
  );

  const handleRemoveFile = useCallback(
    (index: number) => {
      const file = attachedFiles[index];
      if (file) {
        removeAttachedFile(file.filename);
      }
    },
    [attachedFiles, removeAttachedFile],
  );

  return (
    <form ref={chatInputRef} onSubmit={handleSubmit} className={formClassName}>
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <textarea
          value={input}
          onChange={handleAgentInputChange}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          disabled={isLoading || isAttachmentLoading}
          className={`flex-1 min-w-0 resize-none transition-colors bg-transparent outline-none border-none py-2 px-3 text-base leading-relaxed max-h-24 overflow-y-auto ${inputClassName}`}
          style={textareaStyle}
          autoComplete="off"
          spellCheck="false"
          rows={1}
        />

        <FileAttachment
          files={fileAttachmentFiles}
          onRemove={handleRemoveFile}
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
