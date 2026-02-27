import {
  useState,
  useRef,
  useCallback,
  useMemo,
  useEffect,
  useLayoutEffect,
} from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  Button,
  FileAttachment,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { Send, Square, Loader2 } from 'lucide-react';
import type { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { useAgentFileAttachment } from '../hooks/useAgentFileAttachment';
import { useChatSubmit } from '../hooks/useChatSubmit';
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
  const [pendingCancel, setPendingCancel] = useState(false);
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const chatInputRef = useRef<HTMLFormElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
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
    refetchSessionFiles,
  } = useAgentFileAttachment();

  const { input, setInput, isSubmitting, handleSubmit } = useChatSubmit({
    session,
    submit,
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    refetchSessionFiles,
  });

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

  // Auto-resize textarea
  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      const maxHeightPx = 96; // 6rem, matching Tailwind max-h-24
      const nextHeight = Math.min(textarea.scrollHeight, maxHeightPx);
      textarea.style.height = `${nextHeight}px`;
    }
  }, [input]);

  const handleAgentInputChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setInput(e.target.value);
    },
    [setInput],
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

  const inputClassName = cn(
    'flex-1 min-w-0 resize-none transition-colors bg-transparent outline-none border-none py-2 px-3 text-base leading-relaxed max-h-24 overflow-y-auto',
  );

  const formClassName = cn(
    'px-4 py-4 border-t flex items-center gap-2 transition-colors',
    dragState === 'valid' && 'bg-success/10 border-success',
    dragState === 'invalid' && 'bg-destructive/10 border-destructive',
  );

  return (
    <form ref={chatInputRef} onSubmit={handleSubmit} className={formClassName}>
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleAgentInputChange}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          // Always enabled to allow typing while attachments upload
          className={inputClassName}
          style={textareaStyle}
          autoComplete="off"
          spellCheck="false"
          rows={1}
          aria-label="Chat input"
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
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="submit"
              disabled={
                (!input.trim() && attachedFiles.length === 0) ||
                isAttachmentLoading
              }
              variant="ghost"
              size="icon"
              aria-label="Send message"
            >
              <Send className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Send message</TooltipContent>
        </Tooltip>

        {isBusy && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                onClick={handleCancel}
                variant="destructive"
                size="icon"
                disabled={pendingCancel}
                aria-label="Cancel request"
              >
                {pendingCancel ? (
                  <Loader2 className="h-4 w-4 animate-spin text-warning" />
                ) : (
                  <Square className="h-4 w-4" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {pendingCancel ? 'Cancelling...' : 'Cancel request'}
            </TooltipContent>
          </Tooltip>
        )}
      </div>
    </form>
  );
}
