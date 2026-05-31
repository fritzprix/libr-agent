import { useState, useCallback } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { useSettings } from '@/hooks/use-settings';
import type { Message, AttachmentReference } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import type { useAgentChat } from '@/context/AgentChatContext';
import type { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';

const logger = getLogger('useChatSubmit');
const OBVIOUS_OVERSIZE_CHAR_MULTIPLIER = 2;

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
  onSubmitted?: () => void | Promise<void>;
}

export function useChatSubmit({
  session,
  submit,
  pendingFiles,
  commitPendingFiles,
  clearPendingFiles,
  refetchSessionFiles,
  hasPersistedMessages,
  onSubmitted,
}: UseChatSubmitProps) {
  const { value: settings } = useSettings();
  const [input, setInput] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = useCallback(
    async (e?: React.FormEvent<HTMLFormElement>) => {
      e?.preventDefault();

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
      settings.maxInputContext,
    ],
  );

  return {
    input,
    setInput,
    isSubmitting,
    handleSubmit,
  };
}
