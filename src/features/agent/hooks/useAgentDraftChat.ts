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
import {
  addAgentAttachment,
  toWorkspaceOnlyAttachment,
} from '../lib/resource-attachment-operations';
import {
  getDockerNotAvailableMessage,
  isDockerNotAvailableError,
} from '@/lib/backend/errors';

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
  const [input, setInput] = useState(
    () => searchParams.get('prompt') || searchParams.get('initialInput') || '',
  );
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
  const [workspaceIsolation, setWorkspaceIsolation] = useState<
    'host' | 'docker'
  >('host');
  const [dockerImage, setDockerImage] = useState<string>('python:3.11-slim');
  const [dockerError, setDockerError] = useState<string | null>(null);
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const [profileDragState, setProfileDragState] = useState<
    'none' | 'valid' | 'invalid'
  >('none');
  const [isAttachmentLoading, setIsAttachmentLoading] = useState(false);

  const formRef = useRef<HTMLFormElement>(null);
  const profileAreaRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const provisioningToastRef = useRef<{
    id: string | number;
    sessionId: string;
  } | null>(null);
  const autoSubmitRef = useRef(false);

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

  useEffect(() => {
    const promptParam =
      searchParams.get('prompt') || searchParams.get('initialInput');
    if (promptParam && !input) {
      setInput(promptParam);
    }
  }, [searchParams, input]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<AgentEventPayload>('agent:event', (event) => {
      const payload = event.payload;
      if (payload.type !== 'sessionRuntimeStateUpdated') {
        return;
      }

      const activeToast = provisioningToastRef.current;
      if (!activeToast || payload.sessionId !== activeToast.sessionId) {
        return;
      }

      const step =
        payload.runtimeState.initialization.docker?.step ??
        payload.runtimeState.initialization.currentStep;
      if (!step) {
        return;
      }

      toast.loading(step, { id: activeToast.id });
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const submitDraft = useCallback(async () => {
    if (
      (!input.trim() && pendingFiles.length === 0) ||
      !assistant ||
      isSubmitting
    )
      return;

    if (workspaceIsolation === 'docker' && !dockerImage.trim()) {
      toast.error(t('agent.draft.dockerImageRequired'));
      return;
    }

    setIsSubmitting(true);
    const newSessionId = createId();
    const now = new Date();
    let toastId: string | number | undefined;

    const resolvedInput = input.trim();
    const shortName =
      resolvedInput.length > 50
        ? resolvedInput.substring(0, 47) + '...'
        : resolvedInput;
    const filesToAttach = [...pendingFiles];

    try {
      toastId = toast.loading(
        workspaceIsolation === 'docker'
          ? t('agent.draft.dockerCreating', {
              image: dockerImage,
              defaultValue:
                'Creating session — preparing Docker image {{image}}…',
            })
          : t('agent.draft.creatingSession'),
      );
      provisioningToastRef.current = { id: toastId, sessionId: newSessionId };

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
          model: overrideModel ?? settings?.preferredModel?.model ?? 'gpt-4',
          provider:
            overrideProvider ?? settings?.preferredModel?.provider ?? 'openai',
          agentConfig,
          isEphemeral: false,
          workspacePath: workspaceOverride || undefined,
          workspaceIsolation: workspaceIsolation,
          dockerConfig:
            workspaceIsolation === 'docker'
              ? {
                  image: dockerImage,
                }
              : undefined,
        },
      });

      const attachments: AttachmentReference[] = [];
      for (const file of filesToAttach) {
        try {
          const result = await addAgentAttachment({
            sessionId: newSessionId,
            url: '',
            mimeType: file.type || getMimeType(file.name),
            filename: file.name,
            file,
            inlineAudio:
              settings?.experimental?.inlineAudioAttachment !== false,
          });
          attachments.push(result);
        } catch (err) {
          logger.error(
            'Failed to attach draft file, falling back to workspace-only',
            {
              filename: file.name,
              err,
            },
          );
          toast.error(t('agent.draft.failedToAttach', { file: file.name }));
          try {
            const fallback = toWorkspaceOnlyAttachment(
              newSessionId,
              file.name,
              file.type || getMimeType(file.name),
              file.size,
            );
            attachments.push(fallback);
          } catch (fallbackErr) {
            logger.error(
              'Failed to create fallback workspace-only attachment',
              fallbackErr,
            );
          }
        }
      }

      // Filter out valid inline attachments that successfully materialized base64 data to be sent inside message.content
      const validInlineRefs = attachments.filter(
        (r) => r.status === 'inline' && r.inlineContent && r.inlineContent.data,
      );
      // All other attachments (text, workspace-only, or inline attachments that failed inline data generation)
      // should be placed in the message.attachments array.
      const nonInlineRefs = attachments.filter(
        (r) =>
          r.status !== 'inline' || !r.inlineContent || !r.inlineContent.data,
      );

      const inlineContent = validInlineRefs.map((r) => {
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

      const initialMessage: Message = {
        id: createId(),
        sessionId: newSessionId,
        threadId: newSessionId,
        role: 'user',
        content: [{ type: 'text', text: resolvedInput }, ...inlineContent],
        createdAt: now,
        updatedAt: now,
      };

      if (nonInlineRefs.length > 0) {
        initialMessage.attachments = nonInlineRefs;
      }

      const rustMessage = {
        ...initialMessage,
        createdAt: now.getTime(),
        updatedAt: now.getTime(),
      };

      const finalRustMessage = {
        ...rustMessage,
        ...(nonInlineRefs.length > 0 ? { attachments: nonInlineRefs } : {}),
      };

      try {
        await safeInvoke<AgentResponse>('agent_send_message', {
          request: {
            sessionId: newSessionId,
            message: finalRustMessage,
          },
        });
      } catch (sendError) {
        logger.error('Failed to send initial draft message', sendError);
        if (toastId) toast.dismiss(toastId);
        provisioningToastRef.current = null;
        setInput(resolvedInput);
        setPendingFiles(filesToAttach);
        toast.error(t('agent.draft.failedToStartSession'));
        setIsSubmitting(false);
        return;
      }

      setInput('');
      setPendingFiles([]);

      if (toastId) {
        toast.dismiss(toastId);
      }
      provisioningToastRef.current = null;

      navigate(`/agent/${newSessionId}`);
      setIsSubmitting(false);
    } catch (err) {
      if (toastId) toast.dismiss(toastId);
      provisioningToastRef.current = null;
      logger.error('Failed to create draft session', err);

      // Restore input and files on failure so user doesn't lose their draft message/files
      setInput(resolvedInput);
      setPendingFiles(filesToAttach);

      const errMsg = getDockerNotAvailableMessage(err);
      if (isDockerNotAvailableError(err)) {
        setDockerError(errMsg);
      } else {
        toast.error(t('agent.draft.failedToStartSession'));
      }
      setIsSubmitting(false);
    }
  }, [
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
    workspaceIsolation,
    dockerImage,
    t,
  ]);

  useEffect(() => {
    if (
      searchParams.get('autoSubmit') === 'true' &&
      !isLoadingAssistant &&
      assistant &&
      input.trim() &&
      !isSubmitting &&
      !autoSubmitRef.current
    ) {
      autoSubmitRef.current = true;
      void submitDraft();
    }
  }, [
    searchParams,
    isLoadingAssistant,
    assistant,
    input,
    isSubmitting,
    submitDraft,
  ]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      await submitDraft();
    },
    [submitDraft],
  );

  const retryDraftSubmit = useCallback(async () => {
    await submitDraft();
  }, [submitDraft]);

  return {
    assistant,
    setAssistant,
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
    workspaceIsolation,
    setWorkspaceIsolation,
    dockerImage,
    setDockerImage,
    dragState,
    profileDragState,
    isAttachmentLoading,
    formRef,
    profileAreaRef,
    textareaRef,
    handleFileAdd,
    handleFileRemove,
    addFiles,
    handleSubmit,
    retryDraftSubmit,
    stage,
    typeResults,
    skillResults,
    onInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
    dockerError,
    setDockerError,
  };
}
