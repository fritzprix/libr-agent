import { useState, useCallback, useRef } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { useSettings } from '@/hooks/use-settings';
import type { Message, AttachmentReference } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import type { useAgentChat } from '@/context/AgentChatContext';
import type { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { safeInvoke } from '@/lib/backend/core';
import type { ExecutionMode } from '@/context/agent-session/types';
import { isExecutionMode } from '@/lib/generated/execution-mode';

const logger = getLogger('useChatSubmit');
const OBVIOUS_OVERSIZE_CHAR_MULTIPLIER = 2;

function parsePermissionCommand(commandText: string): ExecutionMode | null {
  const match = /^\/permission\s+(yolo|unsafe|normal)$/i.exec(
    commandText.trim(),
  );
  if (!match) {
    return null;
  }

  const mode = match[1].toLowerCase();
  return isExecutionMode(mode) ? mode : null;
}

interface UseChatSubmitProps {
  session: { id: string; threadId?: string } | null;
  submit: ReturnType<typeof useAgentChat>['submit'];
  pendingFiles: ReturnType<typeof useAgentResourceAttachment>['pendingFiles'];
  commitPendingFiles: ReturnType<
    typeof useAgentResourceAttachment
  >['commitPendingFiles'];
  clearPendingFiles: ReturnType<
    typeof useAgentResourceAttachment
  >['clearPendingFiles'];
  refetchSessionFiles: ReturnType<
    typeof useAgentResourceAttachment
  >['refetchSessionFiles'];
  hasPersistedMessages: boolean;
  /** When false, non-command chat submit is blocked until Session Ready. */
  isProxyReady?: boolean;
  onSubmitted?: () => void | Promise<void>;
  onClearSession?: () => void;
  onExecutionModeChange?: (mode: ExecutionMode) => void;
}

export function useChatSubmit({
  session,
  submit,
  pendingFiles,
  commitPendingFiles,
  clearPendingFiles,
  refetchSessionFiles,
  hasPersistedMessages,
  isProxyReady = true,
  onSubmitted,
  onClearSession,
  onExecutionModeChange,
}: UseChatSubmitProps) {
  const { t } = useTranslation();
  const { value: settings } = useSettings();
  const [input, setInput] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  // Ref avoids stale closure and blocks nested `/clear` while invoke is in flight.
  const isSubmittingRef = useRef(false);

  const handleSubmit = useCallback(
    async (e?: React.FormEvent<HTMLFormElement>) => {
      e?.preventDefault();

      if (isSubmittingRef.current) {
        logger.info('Submit ignored: already submitting');
        return;
      }

      const messageText = input.trim();
      const hasInput = messageText.length > 0 || pendingFiles.length > 0;

      if (!hasInput) {
        logger.info('Submit ignored: no input and no pending files');
        return;
      }
      if (!session?.id) {
        logger.info('Submit ignored: no session');
        return;
      }

      // Intercept CLI command execution (allowed before Session Ready)
      if (messageText.startsWith('/')) {
        isSubmittingRef.current = true;
        setIsSubmitting(true);
        const currentInput = input;
        setInput(''); // Clear input immediately for better UX
        // Optimistic UI clear: do not wait for invoke. Backend also emits
        // resourceUpdated(clear); without this, a slow command looks like a
        // failed clear and users re-issue `/clear` (nested reset race).
        if (messageText === '/clear') {
          onClearSession?.();
          clearPendingFiles();
        }

        try {
          const result = await safeInvoke<{
            success: boolean;
            message: string;
          }>('agent_execute_command', {
            sessionId: session.id,
            commandText: messageText,
          });
          if (result.success) {
            if (messageText !== '/clear') {
              clearPendingFiles();
              const permissionMode = parsePermissionCommand(messageText);
              if (permissionMode) {
                onExecutionModeChange?.(permissionMode);
              }
            }
            toast.success(result.message);
          } else {
            toast.error(result.message || 'Command failed');
            setInput(currentInput); // Restore input on error
          }
        } catch (err) {
          logger.error('Failed to execute command:', err);
          toast.error(
            typeof err === 'string' ? err : 'Command execution failed',
          );
          setInput(currentInput); // Restore input on error
        } finally {
          isSubmittingRef.current = false;
          setIsSubmitting(false);
        }
        return;
      }

      if (!isProxyReady) {
        logger.info(
          'Submit ignored: proxy not ready (MCP discovery unfinished)',
        );
        toast.error(t('agent.input.proxyNotReadyToast'));
        return;
      }

      if (
        !hasPersistedMessages &&
        pendingFiles.length === 0 &&
        messageText.length >
          settings.maxInputContext * OBVIOUS_OVERSIZE_CHAR_MULTIPLIER
      ) {
        logger.warn('Rejected obvious oversize first input in ChatInput', {
          sessionId: session.id,
          inputLength: messageText.length,
          maxInputContext: settings.maxInputContext,
          obviousOversizeCharLimit:
            settings.maxInputContext * OBVIOUS_OVERSIZE_CHAR_MULTIPLIER,
        });
        toast.error(
          'First input is too large to start this session. Split it up or raise Max Input Context in Settings.',
        );
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
          toast.error('Failed to upload files. Please try again.');
          return;
        }
      }

      // Split attachments: inline (image/audio) go straight into message.content;
      // all others stay as attachments for the text-hint preprocessor.
      const inlineRefs = attachedFileRefs.filter((r) => r.status === 'inline');
      const textRefs = attachedFileRefs.filter((r) => r.status !== 'inline');

      // Build multimodal content for inline attachments
      const inlineContent: MCPContent[] = inlineRefs
        .filter((r) => !!r.inlineContent)
        .map((r) => {
          if (r.inlineContent!.type === 'image') {
            return {
              type: 'image' as const,
              data: r.inlineContent!.data,
              uri: r.inlineContent!.uri,
              mimeType: r.inlineContent!.mimeType,
            };
          }
          return {
            type: 'audio' as const,
            data: r.inlineContent!.data,
            uri: r.inlineContent!.uri,
            mimeType: r.inlineContent!.mimeType,
          };
        });

      const userMessage: Message = {
        id: createId(),
        sessionId: session.id,
        threadId: session.id,
        role: 'user',
        content: [{ type: 'text', text: messageText }, ...inlineContent],
        createdAt: new Date(),
        updatedAt: new Date(),
      };

      // Include non-inline attachments for text-hint preprocessor
      if (textRefs.length > 0) {
        userMessage.attachments = textRefs;
      }

      setIsSubmitting(true);
      isSubmittingRef.current = true;
      const currentInput = input;
      setInput(''); // Clear input immediately for better UX
      clearPendingFiles();

      try {
        await submit(userMessage);
        logger.info('Message submitted successfully');

        // Refetch session files after successful message submission
        // This ensures SessionFilesPopover shows updated file count
        if (attachedFileRefs.length > 0) {
          logger.info(
            'Refetching session files after message with attachments',
          );
          refetchSessionFiles().catch((error) => {
            logger.warn(
              'Failed to refetch session files after message submission',
              error,
            );
          });
        }

        await onSubmitted?.();
      } catch (err) {
        // Restore input on error
        setInput(currentInput);
        logger.error('Failed to submit message:', err);
      } finally {
        isSubmittingRef.current = false;
        setIsSubmitting(false);
      }
    },
    [
      input,
      pendingFiles,
      session,
      commitPendingFiles,
      clearPendingFiles,
      submit,
      refetchSessionFiles,
      hasPersistedMessages,
      onSubmitted,
      onClearSession,
      onExecutionModeChange,
      settings.maxInputContext,
      isProxyReady,
      t,
    ],
  );

  return {
    input,
    setInput,
    isSubmitting,
    handleSubmit,
  };
}
