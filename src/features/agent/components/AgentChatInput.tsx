import { useState, useRef, useCallback, useMemo, useEffect } from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useLLMService } from '@/context/LLMServiceContext';
import {
  Button,
  FileAttachment,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { Send, Square, Loader2, Play } from 'lucide-react';
import type { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';
import { useAgentFileAttachment } from '../hooks/useAgentFileAttachment';
import { useChatSubmit } from '../hooks/useChatSubmit';
import { useInputToken } from '../hooks/useInputToken';
import { usePlaybookSearch } from '../hooks/usePlaybookSearch';
import { InputTokenDropdown } from './InputTokenDropdown';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useScopedSkills } from '../hooks/useScopedSkills';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useWorkspaceFiles } from '../hooks/useWorkspaceFiles';
import { useTextareaAutosize } from '@/hooks/useTextareaAutosize';
import { AGENT_ATTACHMENT_PICKER_ACCEPT } from '../lib/attachment-picker';

const logger = getLogger('AgentChatInput');

const textareaStyle = {
  msOverflowStyle: 'none',
  scrollbarWidth: 'none',
} as const;

interface AgentChatInputProps {
  children?: React.ReactNode;
}

export function AgentChatInput({ children }: AgentChatInputProps) {
  const { t } = useTranslation();
  const { session, messages } = useAgentSessionState();
  const { submit, isSessionLoading, workflowStatus, cancel, resume } =
    useAgentChat();
  const { isCompacting } = useLLMService();
  const [pendingCancel, setPendingCancel] = useState(false);
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const chatInputRef = useRef<HTMLFormElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const previousWorkflowStatusRef = useRef(workflowStatus);
  const { subscribe } = useDnDContext();
  const { skills, refresh: refreshSkills } = useScopedSkills({
    assistantId: session?.assistant?.id,
    sessionId: session?.id,
  });
  const { availableTools: tools } = useAgentTools(session?.id);

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

  const playbookQuery =
    stage.kind === 'typing-arg' && stage.typeName === 'playbook'
      ? stage.query
      : null;
  const playbookResults = usePlaybookSearch(
    session?.assistant?.id,
    playbookQuery,
  );

  const {
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    isAttachmentLoading,
    attachFiles,
    handleFileAttachment,
    removeFile,
    processFileDrop,
    validateFiles,
    refetchSessionFiles,
  } = useAgentFileAttachment();

  const refreshScopedSkills = useCallback(async () => {
    await refreshSkills();
  }, [refreshSkills]);

  const { input, setInput, isSubmitting, handleSubmit } = useChatSubmit({
    session,
    submit,
    pendingFiles,
    commitPendingFiles,
    clearPendingFiles,
    refetchSessionFiles,
    hasPersistedMessages: messages.length > 0,
    onSubmitted: refreshScopedSkills,
  });

  const attachedFiles = pendingFiles;
  const hasContent = input.trim().length > 0 || attachedFiles.length > 0;

  const hasProcessingFiles = useMemo(
    () => pendingFiles.some((f) => f.status === 'processing'),
    [pendingFiles],
  );

  // Agent busy state (shown as Cancel button)
  const isBusy = useMemo(() => {
    return (
      isSessionLoading ||
      isSubmitting ||
      workflowStatus === 'busy' ||
      (session?.id ? isCompacting(session.id) : false)
    );
  }, [
    isSessionLoading,
    isSubmitting,
    workflowStatus,
    session?.id,
    isCompacting,
  ]);

  const isPaused = workflowStatus === 'paused' && !isBusy;

  const isSendDisabled = useMemo(() => {
    return (
      !hasContent ||
      isAttachmentLoading ||
      hasProcessingFiles ||
      isSessionLoading
    );
  }, [hasContent, isAttachmentLoading, hasProcessingFiles, isSessionLoading]);

  const inputPlaceholder = useMemo(() => {
    if (dragState !== 'none') {
      return dragState === 'valid'
        ? t('agent.input.placeholderDropValid')
        : t('agent.input.placeholderDropInvalid');
    }
    if (isAttachmentLoading) return t('agent.input.placeholderUploading');

    if (isBusy) {
      return t('agent.input.placeholderBusy');
    }
    return t('agent.input.placeholderDefault');
  }, [dragState, isBusy, isAttachmentLoading, t]);

  useTextareaAutosize({
    textareaRef,
    value: input,
    maxHeight: 96,
  });

  useEffect(() => {
    const previousWorkflowStatus = previousWorkflowStatusRef.current;
    previousWorkflowStatusRef.current = workflowStatus;

    if (
      session?.id &&
      workflowStatus === 'idle' &&
      previousWorkflowStatus !== 'idle'
    ) {
      void refreshScopedSkills();
    }
  }, [refreshScopedSkills, session?.id, workflowStatus]);

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

  const handleFocus = useCallback(() => {
    void refreshScopedSkills();
  }, [refreshScopedSkills]);

  const insertTextAtSelection = useCallback(
    (text: string) => {
      const textarea = textareaRef.current;
      const selectionStart = textarea?.selectionStart ?? input.length;
      const selectionEnd = textarea?.selectionEnd ?? input.length;
      const nextValue =
        input.slice(0, selectionStart) + text + input.slice(selectionEnd);
      const nextCursorPosition = selectionStart + text.length;

      setInput(nextValue);
      onTokenInputChange(nextValue, nextCursorPosition);

      requestAnimationFrame(() => {
        textareaRef.current?.focus();
        textareaRef.current?.setSelectionRange(
          nextCursorPosition,
          nextCursorPosition,
        );
      });
    },
    [input, onTokenInputChange, setInput],
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
          fileResults.length > 0 ||
          playbookResults.length > 0)
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
          !isSendDisabled &&
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
      isSendDisabled,
      isAttachmentLoading,
      input,
      attachedFiles.length,
      stage.kind,
      typeResults.length,
      skillResults.length,
      toolResults.length,
      fileResults.length,
      playbookResults.length,
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

  const handleResume = useCallback(async () => {
    try {
      await resume();
    } catch (err) {
      logger.error('Failed to resume workflow:', err);
    }
  }, [resume]);

  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const clipboardData = e.clipboardData;
      const imageFilesFromItems = Array.from(clipboardData.items)
        .filter(
          (item) => item.kind === 'file' && item.type.startsWith('image/'),
        )
        .flatMap((item) => {
          const file = item.getAsFile();
          return file ? [file] : [];
        });
      const imageFiles =
        imageFilesFromItems.length > 0
          ? imageFilesFromItems
          : Array.from(clipboardData.files).filter((file) =>
              file.type.startsWith('image/'),
            );

      if (imageFiles.length === 0) {
        return;
      }

      e.preventDefault();

      const pastedText = clipboardData.getData('text/plain');
      if (pastedText) {
        insertTextAtSelection(pastedText);
      }

      void attachFiles(imageFiles);
    },
    [attachFiles, insertTextAtSelection],
  );

  // Drag-and-drop handlers
  useEffect(() => {
    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      if (event === 'drag-over') {
        const isValid = payload.paths ? validateFiles(payload.paths) : false;
        setDragState(isValid ? 'valid' : 'invalid');
      } else if (event === 'drop') {
        setDragState('none');
        if (payload.paths) {
          // Defer heavy file processing to unblock the UI thread
          // This ensures the visual drop state resets immediately
          const paths = payload.paths;
          setTimeout(() => {
            processFileDrop(paths);
          }, 0);
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
        status: file.status,
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
    'flex-1 resize-none transition-colors bg-transparent outline-none border-none py-3 px-2 text-sm leading-relaxed max-h-32 min-h-[44px] overflow-y-auto',
  );

  const hasAttachedFiles = attachedFiles.length > 0;

  const formClassName = cn(
    'flex items-end gap-2 border border-border/40 bg-background/45 p-3 shadow-[0_20px_48px_-28px_rgba(0,0,0,0.55)] transition-all supports-[backdrop-filter]:bg-background/30 backdrop-blur-xl focus-within:ring-1 focus-within:ring-primary/20',
    hasAttachedFiles ? 'rounded-b-xl border-t-0' : 'rounded-xl',
    dragState === 'valid' && 'bg-success/5 border-success/50 shadow-success/10',
    dragState === 'invalid' &&
      'bg-destructive/5 border-destructive/50 shadow-destructive/10',
  );

  return (
    <div className="relative">
      {stage.kind !== 'idle' &&
        (typeResults.length > 0 ||
          skillResults.length > 0 ||
          toolResults.length > 0 ||
          fileResults.length > 0 ||
          playbookResults.length > 0) && (
          <InputTokenDropdown
            mode={
              stage.kind === 'typing-type'
                ? { kind: 'types', items: typeResults }
                : stage.typeName === 'tool'
                  ? { kind: 'tools', items: toolResults }
                  : stage.typeName === 'file'
                    ? { kind: 'files', items: fileResults }
                    : stage.typeName === 'playbook'
                      ? { kind: 'playbooks', items: playbookResults }
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
      <form
        ref={chatInputRef}
        onSubmit={handleSubmit}
        className={formClassName}
      >
        <FileAttachment
          files={fileAttachmentFiles}
          onRemove={handleRemoveFile}
          onAdd={handleFileAttachment}
          compact={true}
          accept={AGENT_ATTACHMENT_PICKER_ACCEPT}
        />
        {children}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleAgentInputChange}
          onFocus={handleFocus}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={inputPlaceholder}
          className={inputClassName}
          style={textareaStyle}
          autoComplete="off"
          spellCheck="false"
          rows={1}
          aria-label={t('agent.input.ariaLabel')}
        />
        {isBusy ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <span
                tabIndex={pendingCancel ? 0 : undefined}
                className={cn(
                  'inline-block rounded-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none mb-1 shrink-0 h-8 w-8',
                  pendingCancel && 'cursor-not-allowed',
                )}
                aria-label={
                  pendingCancel ? t('agent.input.cancelAriaLabel') : undefined
                }
                aria-disabled={pendingCancel ? true : undefined}
                role={pendingCancel ? 'button' : undefined}
              >
                <Button
                  type="button"
                  onClick={handleCancel}
                  variant="ghost"
                  size="icon"
                  className={cn(
                    'h-full w-full text-destructive hover:bg-destructive/10',
                    pendingCancel && 'pointer-events-none',
                  )}
                  disabled={pendingCancel}
                  aria-label={t('agent.input.cancelAriaLabel')}
                  title={t('agent.input.cancelAriaLabel')}
                >
                  {pendingCancel ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Square className="h-4 w-4" />
                  )}
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>
              {pendingCancel
                ? t('agent.input.cancellingTooltip')
                : t('agent.input.cancelTooltip')}
            </TooltipContent>
          </Tooltip>
        ) : isPaused ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                onClick={handleResume}
                variant="ghost"
                size="icon"
                className="mb-1 shrink-0"
                aria-label={t('agent.input.resumeAriaLabel')}
                title={t('agent.input.resumeAriaLabel')}
              >
                <Play className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t('agent.input.resumeTooltip')}</TooltipContent>
          </Tooltip>
        ) : (
          <Tooltip>
            <TooltipTrigger asChild>
              <span
                tabIndex={isSendDisabled ? 0 : undefined}
                className={cn(
                  'inline-block rounded-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none mb-1 shrink-0',
                  isSendDisabled && 'cursor-not-allowed',
                )}
                aria-label={
                  isSendDisabled ? t('agent.input.sendAriaLabel') : undefined
                }
                aria-disabled={isSendDisabled ? true : undefined}
                role={isSendDisabled ? 'button' : undefined}
              >
                <Button
                  type="submit"
                  disabled={isSendDisabled}
                  size="icon"
                  className={cn(isSendDisabled && 'pointer-events-none')}
                  aria-label={t('agent.input.sendAriaLabel')}
                  title={t('agent.input.sendAriaLabel')}
                >
                  {hasProcessingFiles ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Send className="h-4 w-4" />
                  )}
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>{t('agent.input.sendTooltip')}</TooltipContent>
          </Tooltip>
        )}
      </form>
    </div>
  );
}
