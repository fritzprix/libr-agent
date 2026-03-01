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
import { useInputToken } from '../hooks/useInputToken';
import { InputTokenDropdown } from './InputTokenDropdown';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useSkills } from '@/context/SkillsContext';
import { useSessionTools } from '../hooks/useSessionTools';
import { useWorkspaceFiles } from '../hooks/useWorkspaceFiles';

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
  const { skills } = useSkills();
  const { tools } = useSessionTools(session?.id);

  const {
    stage,
    typeResults,
    skillResults,
    toolResults,
    onInputChange: onTokenInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
  } = useInputToken(skills, tools);

  // null = dropdown not active; string (incl. '') = active with current query
  const fileQuery =
    stage.kind === 'typing-arg' && stage.typeName === 'file'
      ? stage.query
      : null;
  const fileResults = useWorkspaceFiles(session?.id, fileQuery);

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
    return 'Query agent or drop files... (@ for references)';
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
      onTokenInputChange(
        e.target.value,
        e.target.selectionStart ?? e.target.value.length,
      );
    },
    [setInput, onTokenInputChange],
  );

  // Handle Enter/Shift+Enter for line breaks and submission
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Let InputTokenDropdown handle navigation keys when dropdown is open
      if (
        stage.kind !== 'idle' &&
        (typeResults.length > 0 ||
          skillResults.length > 0 ||
          toolResults.length > 0 ||
          fileResults.length > 0)
      ) {
        if (
          ['ArrowUp', 'ArrowDown', 'Enter', 'Tab', 'Escape'].includes(e.key)
        ) {
          return; // InputTokenDropdown's capture listener has priority
        }
      }
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
    [
      isAttachmentLoading,
      input,
      attachedFiles.length,
      stage.kind,
      typeResults.length,
      skillResults.length,
      toolResults.length,
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

  const inputClassName = cn(
    'w-full resize-none transition-colors bg-transparent outline-none border-none py-3 px-4 text-sm leading-relaxed max-h-36 overflow-y-auto',
  );

  const formClassName = cn(
    'mx-4 mb-4 rounded-2xl border bg-background shadow-sm transition-colors',
    dragState === 'valid' && 'border-success bg-success/5',
    dragState === 'invalid' && 'border-destructive bg-destructive/5',
    dragState === 'none' && 'border-border',
  );

  const hasContent = input.trim() || attachedFiles.length > 0;

  return (
    <form ref={chatInputRef} onSubmit={handleSubmit} className={formClassName}>
      <div className="relative">
        {stage.kind !== 'idle' &&
          (typeResults.length > 0 ||
            skillResults.length > 0 ||
            toolResults.length > 0 ||
            fileResults.length > 0) && (
            <InputTokenDropdown
              mode={
                stage.kind === 'typing-type'
                  ? { kind: 'types', items: typeResults }
                  : stage.typeName === 'tool'
                    ? { kind: 'tools', items: toolResults }
                    : stage.typeName === 'file'
                      ? { kind: 'files', items: fileResults }
                      : { kind: 'skills', items: skillResults }
              }
              onSelectType={(typeName) => {
                const cursorPos =
                  textareaRef.current?.selectionStart ?? input.length;
                const newValue = onTypeSelect(typeName, input, cursorPos);
                setInput(newValue);
                requestAnimationFrame(() => {
                  if (textareaRef.current) {
                    const pos = newValue.length - (input.length - cursorPos);
                    textareaRef.current.setSelectionRange(pos, pos);
                    textareaRef.current.focus();
                  }
                });
              }}
              onSelectArg={(arg) => {
                const cursorPos =
                  textareaRef.current?.selectionStart ?? input.length;
                const newValue = onArgSelect(arg, input, cursorPos);
                setInput(newValue);
                requestAnimationFrame(() => {
                  textareaRef.current?.focus();
                });
              }}
              onDismiss={onDismiss}
            />
          )}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleAgentInputChange}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          className={inputClassName}
          style={textareaStyle}
          autoComplete="off"
          spellCheck="false"
          rows={1}
          aria-label="Chat input"
        />
      </div>

      {/* Bottom toolbar */}
      <div className="flex items-center justify-between px-3 pb-2 gap-2">
        <div className="flex items-center gap-1">
          <FileAttachment
            files={fileAttachmentFiles}
            onRemove={handleRemoveFile}
            onAdd={handleFileAttachment}
            compact={true}
          />
          {children}
        </div>

        <div className="flex items-center gap-1">
          {isBusy ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  onClick={handleCancel}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 rounded-xl text-destructive hover:bg-destructive/10"
                  disabled={pendingCancel}
                  aria-label="Cancel request"
                >
                  {pendingCancel ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Square className="h-4 w-4" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {pendingCancel ? 'Cancelling...' : 'Cancel'}
              </TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="submit"
                  disabled={!hasContent || isAttachmentLoading}
                  size="icon"
                  className="h-8 w-8 rounded-xl"
                  aria-label="Send message"
                >
                  <Send className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Send</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
    </form>
  );
}
