import { useState, useCallback, useEffect, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { createId } from '@paralleldrive/cuid2';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';

import { getLogger } from '@/lib/logger';
import { safeInvoke } from '@/lib/backend/core';
import type { Assistant, Message, AttachmentReference } from '@/models/chat';
import type { AgentResponse, AgentSessionMetadata } from '@/models/agent-ipc';
import type { AssistantDto } from '@/lib/backend/assistants';
import { parseAssistant } from '@/models/validation';
import { useSettings } from '@/context/SettingsContext';
import { enforceRuntimeBuiltinAliases } from '@/lib/assistant/runtime-builtins';
import { checkDroppedPathType } from '@/lib/backend';
import { getMimeTypeFromFilename } from '@/lib/mime-utils';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useInputToken } from './useInputToken';
import { useScopedSkills } from './useScopedSkills';
import type { AgentEventPayload } from '@/context/AgentSessionContext';
import { prepareDraftAttachments } from '../lib/draft-attachments';

const logger = getLogger('useAgentDraftChat');

export interface BuiltinServerInfo {
  name: string; // This is the ID
  metadata: {
    displayName: string;
    description: string;
    icon?: string;
  };
  toolCount: number;
}

export interface MCPServerDto {
  id: string; // This is the name/ID
  name: string;
  config: unknown;
}

export function useAgentDraftChat() {
  const navigate = useNavigate();
  const { t } = useTranslation();

  const { value: settings } = useSettings();
  const [searchParams] = useSearchParams();
  const [assistant, setAssistant] = useState<Assistant | null>(null);
  const [input, setInput] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isLoadingAssistant, setIsLoadingAssistant] = useState(true);

  // Override state for model/provider selection
  const [overrideModel, setOverrideModel] = useState<string | undefined>();
  const [overrideProvider, setOverrideProvider] = useState<
    string | undefined
  >();

  // Metadata state
  const [builtinServices, setBuiltinServices] = useState<BuiltinServerInfo[]>(
    [],
  );
  const [mcpServers, setMcpServers] = useState<MCPServerDto[]>([]);

  // Pre-session file attachments
  const [pendingFiles, setPendingFiles] = useState<File[]>([]);
  const [workspaceOverride, setWorkspaceOverride] = useState<string | null>(
    null,
  );
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const [profileDragState, setProfileDragState] = useState<
    'none' | 'valid' | 'invalid'
  >('none');
  const [isAttachmentLoading, setIsAttachmentLoading] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const profileAreaRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const rustBackend = useRustBackend();
  const { subscribe } = useDnDContext();

  // @skill: mention support
  const { skills } = useScopedSkills(assistant?.id, workspaceOverride);
  const {
    stage,
    typeResults,
    skillResults,
    onInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
  } = useInputToken(skills);

  const getMimeType = getMimeTypeFromFilename;

  const addFiles = useCallback((files: File[]) => {
    if (files.length === 0) return;
    setPendingFiles((prev) => [...prev, ...files]);
  }, []);

  const handleFileAdd = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      addFiles(Array.from(e.target.files ?? []));
      e.target.value = '';
    },
    [addFiles],
  );

  const handleFileRemove = useCallback((index: number) => {
    setPendingFiles((prev) => prev.filter((_, i) => i !== index));
  }, []);

  // Drag-and-drop: read dropped paths via Rust backend, build File objects
  useEffect(() => {
    const processDroppedPaths = (
      paths: string[],
      target: 'profile' | 'form',
    ) => {
      const run = async () => {
        setIsAttachmentLoading(true);
        try {
          try {
            await rustBackend.registerDroppedFiles(paths);
          } catch (err) {
            logger.error('Failed to register dropped files', err);
            toast.error(t('agent.draft.failedToRegisterDroppedFiles'));
            return;
          }
          const files: File[] = [];
          for (const filePath of paths) {
            try {
              const pathType = await checkDroppedPathType(filePath);

              if (target === 'profile') {
                if (pathType === 'directory') {
                  setWorkspaceOverride(filePath);
                } else {
                  toast.error(t('agent.draft.dropFilesInChatInput'));
                }
                continue;
              }

              if (target === 'form') {
                if (pathType === 'directory') {
                  toast.error(t('agent.workspace.dropDirToastError'));
                  continue;
                }
                const fileData = await rustBackend.readDroppedFile(filePath);
                const filename =
                  filePath.split('/').pop() ??
                  filePath.split('\\').pop() ??
                  'unknown';
                const mimeType = getMimeType(filename);
                files.push(
                  new File([new Uint8Array(fileData)], filename, {
                    type: mimeType,
                  }),
                );
              }
            } catch (err) {
              logger.error('Failed to process dropped path', { filePath, err });
              toast.error(
                t('agent.draft.failedToProcessFile', {
                  file: filePath.split(/[\\/]/).pop(),
                }),
              );
            }
          }
          if (files.length > 0) {
            addFiles(files);
          }
        } finally {
          setIsAttachmentLoading(false);
        }
      };
      void run();
    };

    const formHandler = (
      event: DragAndDropEvent,
      payload: DragAndDropPayload,
    ) => {
      if (event === 'drag-over') {
        setDragState(
          payload.paths && payload.paths.length > 0 ? 'valid' : 'invalid',
        );
      } else if (event === 'leave') {
        setDragState('none');
      } else if (event === 'drop') {
        setDragState('none');
        if (payload.paths && payload.paths.length > 0) {
          processDroppedPaths(payload.paths, 'form');
        }
      }
    };

    const profileHandler = (
      event: DragAndDropEvent,
      payload: DragAndDropPayload,
    ) => {
      if (event === 'drag-over') {
        setProfileDragState(
          payload.paths && payload.paths.length > 0 ? 'valid' : 'invalid',
        );
      } else if (event === 'leave') {
        setProfileDragState('none');
      } else if (event === 'drop') {
        setProfileDragState('none');
        if (payload.paths && payload.paths.length > 0) {
          processDroppedPaths(payload.paths, 'profile');
        }
      }
    };

    const unsubForm = subscribe(
      formRef as React.RefObject<HTMLElement>,
      formHandler,
      {
        priority: 5,
      },
    );
    const unsubProfile = subscribe(
      profileAreaRef as React.RefObject<HTMLElement>,
      profileHandler,
      {
        priority: 5,
      },
    );

    return () => {
      unsubForm();
      unsubProfile();
    };
  }, [subscribe, rustBackend, getMimeType, addFiles, t]);

  useEffect(() => {
    const loadMetadata = async () => {
      try {
        const [services, servers] = await Promise.all([
          safeInvoke<BuiltinServerInfo[]>(
            'list_available_builtin_server_definitions',
          ),
          safeInvoke<MCPServerDto[]>('list_mcp_server_configs'),
        ]);
        setBuiltinServices(services);
        setMcpServers(servers);
      } catch (err) {
        logger.error('Failed to load tool metadata', err);
      }
    };
    loadMetadata();
  }, []);

  useEffect(() => {
    const loadAssistant = async () => {
      const assistantId = searchParams.get('assistantId');
      if (!assistantId) {
        toast.error(t('agent.draft.noAssistantSpecified'));
        navigate('/agent/start');
        return;
      }

      try {
        const rawData = await safeInvoke<AssistantDto | null>('get_assistant', {
          id: assistantId,
        });

        if (!rawData) throw new Error('Assistant not found');

        const flattenedAssistant = parseAssistant(rawData);
        setAssistant(flattenedAssistant);
      } catch (err) {
        logger.error('Failed to load assistant', err);
        toast.error(t('agent.draft.failedToLoadAssistant'));
      } finally {
        setIsLoadingAssistant(false);
      }
    };
    loadAssistant();
  }, [searchParams, navigate, t]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (
        (!input.trim() && pendingFiles.length === 0) ||
        !assistant ||
        isSubmitting
      )
        return;

      setIsSubmitting(true);
      const newSessionId = createId();
      const now = new Date();
      let unlisten: (() => void) | undefined;
      let toastId: string | number | undefined;

      const resolvedInput = input.trim();
      const shortName =
        input.trim().length > 50
          ? input.trim().substring(0, 47) + '...'
          : input.trim();

      try {
        unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
          if (
            event.payload.type === 'sessionRuntimeStateUpdated' &&
            event.payload.sessionId === newSessionId
          ) {
            const step = event.payload.runtimeState.initialization.currentStep;
            if (!step) {
              return;
            }
            if (toastId) {
              toast.loading(step, { id: toastId });
            } else {
              toastId = toast.loading(step);
            }
          }
        });

        if (!toastId) toastId = toast.loading(t('agent.draft.creatingSession'));

        const baseSystemPrompt =
          assistant.systemPrompt || 'You are a helpful assistant.';

        const agentConfig = {
          id: assistant.id,
          name: assistant.name,
          description: assistant.description,
          systemPrompt: baseSystemPrompt,
          mcpServerIds: assistant.mcpServerIds || [],
          localServices: assistant.localServices || [],
          allowedBuiltInServiceAliases: enforceRuntimeBuiltinAliases(
            assistant.allowedBuiltInServiceAliases,
          ),
          maxTokens: settings?.advanced?.defaultMaxOutputTokens ?? 8192,
          ...(settings?.advanced?.defaultSessionMaxDepth &&
          settings.advanced.defaultSessionMaxDepth > 0
            ? { maxDepth: settings.advanced.defaultSessionMaxDepth }
            : {}),
          ...(settings?.advanced?.defaultSessionMaxFanout &&
          settings.advanced.defaultSessionMaxFanout > 0
            ? { maxFanout: settings.advanced.defaultSessionMaxFanout }
            : {}),
        };

        // Create session FIRST so workspace/overrides are registered before writing files
        await safeInvoke<AgentSessionMetadata>('agent_create_session', {
          request: {
            sessionId: newSessionId,
            name: shortName,
            model: overrideModel || settings?.preferredModel?.model || 'gpt-4',
            provider:
              overrideProvider ||
              settings?.preferredModel?.provider ||
              'openai',
            agentConfig,
            isEphemeral: false,
            workspacePath: workspaceOverride || undefined,
          },
        });

        const attachments: AttachmentReference[] =
          await prepareDraftAttachments({
            files: pendingFiles,
            sessionId: newSessionId,
            now,
            getMimeType,
            onAttachmentError: (file) => {
              toast.error(t('agent.draft.failedToAttach', { file: file.name }));
            },
          });

        const initialMessage: Message = {
          id: createId(),
          sessionId: newSessionId,
          threadId: newSessionId,
          role: 'user',
          content: [{ type: 'text', text: resolvedInput }],
          createdAt: now,
          updatedAt: now,
          ...(attachments.length > 0 ? { attachments } : {}),
        };

        const rustMessage = {
          ...initialMessage,
          createdAt: now.getTime(),
          updatedAt: now.getTime(),
        };

        const finalRustMessage = {
          ...rustMessage,
          ...(attachments.length > 0 ? { attachments } : {}),
        };
        await safeInvoke<AgentResponse>('agent_send_message', {
          request: {
            sessionId: newSessionId,
            message: finalRustMessage,
          },
        });

        if (toastId) toast.dismiss(toastId);

        navigate(`/agent/${newSessionId}`);
      } catch (err) {
        if (toastId) toast.dismiss(toastId);
        logger.error('Failed to create draft session', err);
        toast.error(t('agent.draft.failedToStartSession'));
        setIsSubmitting(false);
      } finally {
        if (unlisten) unlisten();
      }
    },
    [
      input,
      assistant,
      isSubmitting,
      navigate,
      settings,
      overrideModel,
      overrideProvider,
      pendingFiles,
      getMimeType,
      workspaceOverride,
      t,
    ],
  );

  return {
    assistant,
    isLoadingAssistant,
    input,
    setInput,
    isSubmitting,
    overrideModel,
    setOverrideModel,
    overrideProvider,
    setOverrideProvider,
    builtinServices,
    mcpServers,
    pendingFiles,
    workspaceOverride,
    setWorkspaceOverride,
    dragState,
    profileDragState,
    isAttachmentLoading,
    fileInputRef,
    formRef,
    profileAreaRef,
    textareaRef,
    handleFileAdd,
    handleFileRemove,
    handleSubmit,
    stage,
    typeResults,
    skillResults,
    onInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
  };
}
