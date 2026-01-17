import { useState, useRef, useCallback, useMemo, useEffect } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Button, FileAttachment } from '@/components/ui';
import { Send, Square, Loader2 } from 'lucide-react';
import type { Message, AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useAgentFileAttachment } from '../hooks/useAgentFileAttachment';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';

const logger = getLogger('AgentChatInput');

const textareaStyle = {
  msOverflowStyle: 'none',
  scrollbarWidth: 'none',
} as const;

interface AgentChatInputProps {
  children?: React.ReactNode;
}

export function AgentChatInput({ children }: AgentChatInputProps) {
  const { session } = useAgentSessionState();
  const { submit, isSessionLoading, workflowStatus, cancel } = useAgentChat();
  const [input, setInput] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [pendingCancel, setPendingCancel] = useState(false);
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const chatInputRef = useRef<HTMLFormElement>(null);
  const { subscribe } = useDnDContext();

  const {
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    isAttachmentLoading,
    handleFileAttachment,
    removeFile,
    processFileDrop,
    validateFiles,
  } = useAgentFileAttachment();

  const attachedFiles = pendingFiles;

  // Determine if input should be disabled (busy state detection)
  // Trust backend workflowStatus as single source of truth
  const isBusy = useMemo(() => {
    return (
      isSessionLoading ||
      isSubmitting ||
      workflowStatus === 'busy' ||
      workflowStatus === 'paused'
    );
  }, [isSessionLoading, isSubmitting, workflowStatus]);

  const inputPlaceholder = useMemo(() => {
    if (dragState !== 'none') {
      return dragState === 'valid'
        ? 'Drop supported files here...'
        : 'Unsupported file type!';
    }
    if (isAttachmentLoading) return 'Uploading...';

    if (isBusy) {
      return 'Agent busy. Message will be queued...';
    }
    return 'Query agent or drop files...';
  }, [dragState, isBusy, isAttachmentLoading]);

  const handleAgentInputChange = useCallback(
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
          !isAttachmentLoading &&
          (input.trim() || attachedFiles.length > 0)
        ) {
          chatInputRef.current?.dispatchEvent(
            new Event('submit', { bubbles: true, cancelable: true }),
          );
        }
      }
    },
    [isAttachmentLoading, input, attachedFiles.length],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();

      const hasInput = input.trim().length > 0 || pendingFiles.length > 0;

      if (!hasInput) {
        logger.info('Submit ignored: no input and no pending files');
        return;
      }
      if (!session?.id) {
        logger.info('Submit ignored: no session');
        return;
      }

      let attachedFileRefs: AttachmentReference[] = [];

      if (pendingFiles.length > 0) {
        try {
          logger.info('About to commit pending files', {
            pendingCount: pendingFiles.length,
            filenames: pendingFiles.map((f) => f.filename),
          });
          attachedFileRefs = await commitPendingFiles();
          logger.info('Pending files committed', {
            attachedCount: attachedFileRefs.length,
          });
        } catch (err) {
          logger.error('Error uploading pending files:', err);
          return;
        }
      }

      const userMessage: Message = {
        id: createId(),
        sessionId: session.id,
        threadId: session.id,
        role: 'user',
        content: [{ type: 'text', text: input.trim() }],
        createdAt: new Date(),
        updatedAt: new Date(),
      };

      const supportedFiles = attachedFileRefs.filter((f) => !f.isWorkspaceOnly);
      const workspaceFiles = attachedFileRefs.filter((f) => f.isWorkspaceOnly);

      if (supportedFiles.length > 0) {
        userMessage.attachments = supportedFiles;
      }

      if (workspaceFiles.length > 0) {
        const fileList = workspaceFiles.map((f) => f.filename).join(', ');
        const notice = `I have uploaded the following files to the workspace: ${fileList}`;
        const originalText = (userMessage.content[0] as { text: string }).text;
        const separator = originalText ? '\n\n' : '';
        (userMessage.content[0] as { text: string }).text =
          `${originalText}${separator}${notice}`;
      }

      setIsSubmitting(true);
      const currentInput = input;
      setInput(''); // Clear input immediately for better UX
      clearPendingFiles();

      try {
        await submit(userMessage);
        logger.info('Message submitted successfully');
      } catch (err) {
        // Restore input on error
        setInput(currentInput);
        logger.error('Failed to submit message:', err);
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      input,
      pendingFiles,
      session?.id,
      commitPendingFiles,
      clearPendingFiles,
      submit,
    ],
  );

  const handleCancel = useCallback(async () => {
    setPendingCancel(true);
    try {
      await cancel();
      logger.info('Workflow cancelled successfully');
    } catch (err) {
      logger.error('Failed to cancel workflow:', err);
    } finally {
      setPendingCancel(false);
    }
  }, [cancel]);

  // Drag-and-drop handlers
  useEffect(() => {
    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      if (event === 'drag-over') {
        logger.info('Drag Over', { event, payload });
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

  const removeAttachedFile = useCallback(
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

  const inputClassName = useMemo(() => {
    return `flex-1 min-w-0 resize-none transition-colors bg-transparent outline-none border-none py-2 px-3 text-base leading-relaxed max-h-24 overflow-y-auto ${
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

  return (
    <form ref={chatInputRef} onSubmit={handleSubmit} className={formClassName}>
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <textarea
          value={input}
          onChange={handleAgentInputChange}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          disabled={isAttachmentLoading}
          className={inputClassName}
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
            (!input.trim() && attachedFiles.length === 0) || isAttachmentLoading
          }
          variant="ghost"
          size="icon"
          title="Send message"
        >
          <Send className="h-4 w-4" />
        </Button>

        {isBusy && (
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
