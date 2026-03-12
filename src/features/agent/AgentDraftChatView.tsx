import { safeInvoke } from '@/lib/backend/core';
import { useState, useCallback, useEffect, useRef } from 'react';
import { useNavigate, useSearchParams, Link } from 'react-router-dom';
import { createId } from '@paralleldrive/cuid2';
import { useTranslation } from 'react-i18next';

import { listen } from '@tauri-apps/api/event';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import {
  Button,
  Badge,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import type { AgentEventPayload } from '@/context/AgentSessionContext';
import { AgentModelPicker } from './components/AgentModelPicker';
import {
  Send,
  Square,
  Loader2,
  Bot,
  Brain,
  Globe,
  Database,
  FolderOpen,
  MapPin,
  Puzzle,
  Paperclip,
  X,
} from 'lucide-react';
import type { Assistant, Message } from '@/models/chat';
import type { AgentResponse, AgentSessionMetadata } from '@/models/agent-ipc';
import type { AssistantDto } from '@/lib/backend/assistants';
import { parseAssistant } from '@/models/validation';
import { useSettings } from '@/context/SettingsContext';
import { cn } from '@/lib/utils';
import {
  enforceRuntimeBuiltinAliases,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '@/lib/assistant/runtime-builtins';
import {
  workspaceWriteFile,
  getWorkspaceDir,
  checkDroppedPathType,
} from '@/lib/backend';
import { generateWorkspacePath } from '@/lib/workspace-sync-service';
import { getMimeTypeFromFilename } from '@/lib/mime-utils';
import type { AttachmentReference } from '@/models/chat';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { saveAgentFile } from '@/features/agent/api/agent-backend';
import type { ContentStoreItem } from '@/models/content-store';
import { useSkills } from '@/context/SkillsContext';
import { useInputToken } from './hooks/useInputToken';
import { InputTokenDropdown } from './components/InputTokenDropdown';

const logger = getLogger('AgentDraftChatView');

// File extensions treated as plain text for Content Store indexing
const TEXT_EXTENSIONS_DRAFT =
  /\.(txt|md|markdown|json|jsonc|json5|yaml|yml|toml|js|jsx|ts|tsx|mjs|cjs|py|rb|rs|go|java|c|cpp|h|hpp|css|scss|less|html|htm|svg|sh|bash|zsh|fish|ps1|sql|graphql|csv|log|xml|proto)$/i;

// Binary file extensions that the backend document parser can handle via file:// URL
const BINARY_INDEXABLE_EXTENSIONS_DRAFT = /\.(pdf|docx|xlsx)$/i;

interface BuiltinServerInfo {
  name: string; // This is the ID
  metadata: {
    displayName: string;
    description: string;
    icon?: string;
  };
  toolCount: number;
}

interface MCPServerDto {
  id: string; // This is the name/ID
  name: string;
  config: unknown;
}

// Icon mapping helper (since backend returns string IDs)
const getIconForService = (iconId?: string) => {
  switch (iconId) {
    case 'globe':
      return Globe;
    case 'database':
      return Database;
    case 'brain':
      return Brain;
    case 'folder-open':
      return FolderOpen;
    case 'layout':
      return Square; // Placeholder for UI
    case 'server':
      return Puzzle;
    case 'book':
      return Brain;
    case 'bot':
      return Bot;
    default:
      return Square;
  }
};

function DraftChatInner() {
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

  // Pre-session file attachments (written to workspace before session creation)
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
  const { skills } = useSkills();
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
      // Fire-and-forget: errors handled internally
      const run = async () => {
        setIsAttachmentLoading(true);
        try {
          // Must register paths with Tauri security layer before reading
          try {
            await rustBackend.registerDroppedFiles(paths);
          } catch (err) {
            logger.error('Failed to register dropped files', err);
            toast.error('Failed to register dropped files');
            return;
          }
          const files: File[] = [];
          for (const filePath of paths) {
            try {
              // Check if path is a directory
              const pathType = await checkDroppedPathType(filePath);

              if (target === 'profile') {
                if (pathType === 'directory') {
                  setWorkspaceOverride(filePath);
                } else {
                  toast.error('Drop files in the chat input to attach them.');
                }
                continue;
              }

              if (target === 'form') {
                if (pathType === 'directory') {
                  toast.error(t('agent.workspace.dropDirToastError'));
                  continue;
                }

                // It's a file, read it
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
              logger.error('Failed to process dropped path', {
                filePath,
                err,
              });
              toast.error(
                `Failed to process: ${filePath.split(/[\\/]/).pop()}`,
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
  }, [subscribe, rustBackend, getMimeType, addFiles]);

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
        toast.error('No assistant specified');
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
        toast.error('Failed to load assistant');
      } finally {
        setIsLoadingAssistant(false);
      }
    };
    loadAssistant();
  }, [searchParams, navigate]);

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
      // Temporary state for submission progress
      let unlisten: (() => void) | undefined;
      let toastId: string | number | undefined;

      // @mention references (@skill:, @file:, @playbook:) are resolved by the Rust backend
      // just before LLM submission via ReferenceRegistry. Store the original input as-is.
      const resolvedInput = input.trim();

      const shortName =
        input.trim().length > 50
          ? input.trim().substring(0, 47) + '...'
          : input.trim();

      try {
        // Setup temporary listener for initialization steps BEFORE creating session
        unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
          if (
            event.payload.type === 'initializationStep' &&
            event.payload.sessionId === newSessionId
          ) {
            const step = event.payload.step;
            if (toastId) {
              toast.loading(step, { id: toastId });
            } else {
              toastId = toast.loading(step);
            }
          }
        });

        // Write pending files to workspace before session creation.
        // The workspace directory is created on demand by session_id — no active session needed.
        const attachments: AttachmentReference[] = [];
        for (const file of pendingFiles) {
          try {
            const workspacePath = generateWorkspacePath(file.name);
            const arrayBuffer = await file.arrayBuffer();
            const bytes = Array.from(new Uint8Array(arrayBuffer));
            await workspaceWriteFile(workspacePath, bytes, newSessionId);
            // Count lines for text-based files so the bubble displays correctly.
            // file.type is unreliable for many extensions (e.g. .md, .ts → ''),
            // so fall back to filename extension check.
            let lineCount = 0;
            const isText =
              /^text\/|\/(json|xml|javascript|typescript)/.test(file.type) ||
              TEXT_EXTENSIONS_DRAFT.test(file.name);
            if (isText) {
              try {
                const text = await file.text();
                lineCount = text.split('\n').length;
              } catch {
                // non-critical, leave as 0
              }
            }
            // Determine actual MIME: file.type is already set via getMimeType() for
            // drag-drop paths, but may be empty for native file picker on some platforms.
            const actualMimeType =
              file.type && file.type !== 'application/octet-stream'
                ? file.type
                : getMimeType(file.name);

            const isInlineType =
              actualMimeType.startsWith('image/') ||
              actualMimeType.startsWith('audio/');

            if (isInlineType) {
              // Base64-encode inline for direct LLM consumption.
              // Use chunk-by-chunk conversion to avoid call-stack overflow.
              const byteArray = new Uint8Array(arrayBuffer);
              let binary = '';
              for (let bi = 0; bi < byteArray.length; bi++) {
                binary += String.fromCharCode(byteArray[bi]);
              }
              const base64Data = btoa(binary);
              const inlineType = actualMimeType.startsWith('image/')
                ? ('image' as const)
                : ('audio' as const);
              attachments.push({
                sessionId: newSessionId,
                filename: file.name,
                mimeType: actualMimeType,
                size: file.size,
                lineCount: 0,
                preview: file.name,
                uploadedAt: now.toISOString(),
                status: 'inline',
                workspacePath,
                inlineContent: {
                  type: inlineType,
                  data: base64Data,
                  mimeType: actualMimeType,
                },
              });
            } else {
              attachments.push({
                sessionId: newSessionId,
                filename: file.name,
                mimeType: actualMimeType,
                size: file.size,
                lineCount,
                preview: file.name,
                uploadedAt: now.toISOString(),
                status: 'workspace-only',
                workspacePath,
              });
            }
          } catch (err) {
            logger.error('Failed to write pre-session attachment', err);
            toast.error(`Failed to attach: ${file.name}`);
          }
        }

        // Prepare message
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

        // Prepare Rust-compatible message
        const rustMessage = {
          ...initialMessage,
          createdAt: now.getTime(),
          updatedAt: now.getTime(),
        };

        // System prompt is built in Rust via ContextProvider framework
        // (includes time/location, skills, and other dynamic context)
        const baseSystemPrompt =
          assistant.systemPrompt || 'You are a helpful assistant.';

        // Prepare Config
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

        if (!toastId) toastId = toast.loading('Creating session...');

        // Step 1: Create session — this initializes MCPServiceProxy + Content Store
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

        // Step 2: Commit indexable files to Content Store now that session is initialized
        let workspaceDirCache: string | null = null;
        const getWorkspaceDirCached = async (): Promise<string> => {
          if (workspaceDirCache === null) {
            workspaceDirCache = await getWorkspaceDir(newSessionId);
          }
          return workspaceDirCache!;
        };

        for (let i = 0; i < pendingFiles.length; i++) {
          const file = pendingFiles[i];
          const isTextFile =
            TEXT_EXTENSIONS_DRAFT.test(file.name) || /^text\//.test(file.type);
          const isBinaryIndexable = BINARY_INDEXABLE_EXTENSIONS_DRAFT.test(
            file.name,
          );

          if (isTextFile) {
            try {
              const content = await file.text();
              const result = (await saveAgentFile(newSessionId, file.name, {
                content,
                metadata: {
                  mimeType: file.type || 'text/plain',
                  size: file.size,
                  uploadedAt: now.toISOString(),
                  filename: file.name,
                },
              })) as ContentStoreItem;
              if (result?.contentId) {
                attachments[i] = {
                  ...attachments[i],
                  status: 'committed',
                  contentId: result.contentId,
                  lineCount: result.lineCount ?? attachments[i].lineCount,
                };
              }
            } catch (commitErr) {
              logger.warn(
                'Failed to commit file to Content Store, keeping workspace-only',
                { filename: file.name, error: commitErr },
              );
            }
          } else if (isBinaryIndexable) {
            // Binary files (PDF, DOCX, XLSX) are parsed on the backend via file:// URL
            try {
              const workspaceDir = await getWorkspaceDirCached();
              const workspacePath = attachments[i].workspacePath;
              if (!workspacePath) {
                logger.warn(
                  'Binary file missing workspacePath, skipping index',
                  {
                    filename: file.name,
                  },
                );
                continue;
              }
              // Normalize path separators to forward slashes for file:// URL construction
              const normalizedDir = workspaceDir.replace(/\\/g, '/');
              const normalizedRelative = workspacePath.replace(/\\/g, '/');
              const fileUrl = `file:///${normalizedDir.replace(/^\//, '')}/${normalizedRelative}`;
              const result = (await saveAgentFile(newSessionId, file.name, {
                fileUrl,
                metadata: {
                  mimeType: file.type || 'application/octet-stream',
                  size: file.size,
                  uploadedAt: now.toISOString(),
                  filename: file.name,
                },
              })) as ContentStoreItem;
              if (result?.contentId) {
                attachments[i] = {
                  ...attachments[i],
                  status: 'committed',
                  contentId: result.contentId,
                  lineCount: result.lineCount ?? attachments[i].lineCount,
                };
              }
            } catch (commitErr) {
              logger.warn(
                'Failed to commit binary file to Content Store, keeping workspace-only',
                { filename: file.name, error: commitErr },
              );
            }
          }
        }

        // Step 3: Send initial message with final (possibly committed) attachment refs
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

        // Navigate to persistent view
        // The session now exists and is "Busy" processing the message
        navigate(`/agent/${newSessionId}`);
      } catch (err) {
        if (toastId) toast.dismiss(toastId);
        logger.error('Failed to create draft session', err);
        toast.error('Failed to start session');
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
      skills,
    ],
  );

  if (isLoadingAssistant) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (!assistant) return null;

  const effectiveBuiltinAliases = enforceRuntimeBuiltinAliases(
    assistant.allowedBuiltInServiceAliases,
  );

  const enabledOptionalAliases = effectiveBuiltinAliases.filter((alias) =>
    OPTIONAL_BUILTIN_SERVICE_ALIASES.includes(
      alias as (typeof OPTIONAL_BUILTIN_SERVICE_ALIASES)[number],
    ),
  );

  return (
    <div className="h-full w-full font-mono flex rounded-lg overflow-hidden shadow-2xl flex-col">
      {/* Header */}
      {/* We need to wrap Header or pass props. AgentChatHeader uses context. 
              Refactoring Header to accept props is best, or mock the context.
              For simplicity now, let's just render a simple header or mock context provider?
              A simple custom header is cleaner for Draft View to avoid Context hell.
          */}
      <div className="flex items-center justify-between px-6 py-4 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex items-center gap-3">
          <div className="flex flex-col">
            <span className="font-semibold text-lg">{assistant.name}</span>
            <span className="text-xs text-muted-foreground">{t('agent.draft.newSession')}</span>
          </div>
        </div>
      </div>

      {/* Assistant Profile Card */}
      <div
        ref={profileAreaRef}
        className={cn(
          'flex-1 p-8 flex flex-col items-center justify-center text-center gap-6 overflow-y-auto no-scrollbar transition-all',
          profileDragState === 'valid' &&
            'bg-primary/5 ring-2 ring-primary/30 ring-inset',
          profileDragState === 'invalid' &&
            'bg-destructive/10 ring-2 ring-destructive/30 ring-inset',
        )}
      >
        {/* Identity Section */}
        <div className="flex flex-col items-center space-y-4">
          <div className="w-20 h-20 bg-primary/10 rounded-xl flex items-center justify-center shadow-sm">
            <Bot className="w-10 h-10 text-primary" />
          </div>
          <div className="space-y-2 max-w-lg">
            <h1 className="text-3xl font-bold tracking-tight text-foreground">
              {assistant.name}
            </h1>
            {assistant.description && (
              <p className="text-muted-foreground text-sm leading-relaxed">
                {assistant.description}
              </p>
            )}
          </div>
        </div>
        {/* Workspace Override - Prominent Indicator */}
        {workspaceOverride ? (
          <div className="w-full max-w-md animate-in fade-in zoom-in duration-300">
            <div className="bg-primary/5 border border-primary/20 rounded-xl p-4 flex items-start gap-4 shadow-sm relative overflow-hidden group">
              <div className="absolute top-0 right-0 p-2 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  type="button"
                  onClick={() => setWorkspaceOverride(null)}
                  className="bg-background/80 hover:bg-background rounded-full p-1 shadow-sm border"
                  title="Remove workspace override"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
              <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center shrink-0">
                <FolderOpen className="w-6 h-6 text-primary" />
              </div>
              <div className="flex-1 text-left min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-sm font-bold text-primary uppercase tracking-tight">
                    {t('agent.draft.workspaceOverrideActive')}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground truncate font-mono bg-muted/50 px-2 py-1 rounded">
                  {workspaceOverride}
                </p>
                <p className="text-[10px] text-muted-foreground/70 mt-2 leading-tight">
                  {t('agent.draft.workspaceOverrideDescription')}
                </p>
              </div>
            </div>
          </div>
        ) : (
          <div
            className={cn(
              'text-xs text-muted-foreground transition-opacity duration-300',
              profileDragState === 'valid'
                ? 'opacity-100 font-medium'
                : 'opacity-0 h-0 overflow-hidden',
            )}
          >
            {t('agent.workspace.dropFolderHint')}
          </div>
        )}
        {/* Capabilities Grid */}{' '}
        <div className="flex flex-wrap gap-2 justify-center max-w-2xl mt-2">
          {/* Built-in Tools */}
          <Tooltip delayDuration={300}>
            <TooltipTrigger asChild>
              <div className="cursor-help">
                <Badge
                  variant="secondary"
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-normal"
                >
                  <Square size={12} className="opacity-70" />
                  {t('agent.draft.basicTools')}
                </Badge>
              </div>
            </TooltipTrigger>
            <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-md border">
              <p>
                {t('agent.draft.basicToolsDescription')}
              </p>
            </TooltipContent>
          </Tooltip>

          {enabledOptionalAliases.map((alias) => {
            const info = builtinServices.find((s) => s.name === alias);
            const label = info?.metadata.displayName || alias;
            const Icon = getIconForService(info?.metadata.icon);

            return (
              <Tooltip key={alias} delayDuration={300}>
                <TooltipTrigger asChild>
                  <div className="cursor-help">
                    <Badge
                      variant="secondary"
                      className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-normal"
                    >
                      <Icon size={12} className="opacity-70" />
                      {label}
                    </Badge>
                  </div>
                </TooltipTrigger>
                {info?.metadata.description && (
                  <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-md border">
                    <p>{info.metadata.description}</p>
                  </TooltipContent>
                )}
              </Tooltip>
            );
          })}

          {/* External MCP Servers */}
          {assistant.mcpServerIds?.map((serverId) => {
            // Resolve display name from fetched MCP servers
            const serverConfig = mcpServers.find((s) => s.id === serverId); // ID is Name in current schema
            const label = serverConfig?.name || serverId;

            return (
              <Badge
                key={serverId}
                variant="outline"
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-normal border-dashed"
              >
                <Puzzle size={12} className="opacity-70" />
                {label}
              </Badge>
            );
          })}

          {/* Add Tools Button - Always visible to encourage exploration */}
          <Link to="/assistants">
            <Tooltip delayDuration={300}>
              <TooltipTrigger asChild>
                <Badge
                  variant="outline"
                  className="text-xs text-muted-foreground opacity-50 border-dashed font-normal cursor-pointer hover:opacity-100 hover:bg-muted transition-all"
                >
                  {t('agent.draft.addTools')}
                </Badge>
              </TooltipTrigger>
              <TooltipContent className="mb-1 bg-popover text-popover-foreground border shadow-md">
                <p>{t('agent.draft.addMoreCapabilities')}</p>
              </TooltipContent>
            </Tooltip>
          </Link>
        </div>
        {/* Configuration Footer */}
        <div className="flex flex-col items-center gap-3 mt-4 pt-4 border-t border-border/40 w-full max-w-md">
          {/* Model Picker */}
          <AgentModelPicker
            currentModel={
              overrideModel || settings?.preferredModel?.model || 'gpt-4'
            }
            currentProvider={
              overrideProvider || settings?.preferredModel?.provider || 'openai'
            }
            onConfigUpdate={(model, provider) => {
              setOverrideModel(model);
              setOverrideProvider(provider);
            }}
            className="w-full max-w-xs"
          />

          {/* Local Context Indicator */}
          <div
            className="flex items-center gap-1.5 text-xs uppercase tracking-wider text-muted-foreground/60 font-semibold"
            title="Local Context Injection Active"
          >
            <MapPin size={10} />
            {t('agent.draft.localContext')}
          </div>
        </div>
      </div>

      {/* Simplified Input Area */}
      <div className="p-4 border-t">
        {/* Pending file chips */}
        {pendingFiles.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-1 pb-2">
            {pendingFiles.map((file, index) => (
              <div
                key={`${file.name}-${file.lastModified}-${file.size}`}
                className="flex items-center gap-1 text-xs bg-muted rounded px-2 py-1 max-w-[200px]"
              >
                <Paperclip className="h-3 w-3 shrink-0 text-muted-foreground" />
                <span className="truncate">{file.name}</span>
                <button
                  type="button"
                  onClick={() => handleFileRemove(index)}
                  className="shrink-0 text-muted-foreground hover:text-foreground ml-0.5"
                  aria-label={`Remove ${file.name}`}
                >
                  <X className="h-3 w-3" />
                </button>
              </div>
            ))}
          </div>
        )}
        <div className="relative">
          {/* @skill: mention dropdown */}
          {stage.kind !== 'idle' &&
            (typeResults.length > 0 || skillResults.length > 0) && (
              <InputTokenDropdown
                mode={
                  stage.kind === 'typing-type'
                    ? { kind: 'types', items: typeResults }
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
                  requestAnimationFrame(() => textareaRef.current?.focus());
                }}
                onDismiss={onDismiss}
              />
            )}
          <form
            ref={formRef}
            onSubmit={handleSubmit}
            className={cn(
              'flex items-end gap-2 bg-muted/30 p-2 rounded-lg border focus-within:ring-1 focus-within:ring-primary/20',
              dragState === 'valid' && 'bg-success/10 border-success',
              dragState === 'invalid' && 'bg-destructive/10 border-destructive',
            )}
          >
            {/* Hidden file input */}
            <input
              ref={fileInputRef}
              type="file"
              multiple
              onChange={handleFileAdd}
              className="hidden"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => fileInputRef.current?.click()}
              disabled={isSubmitting || isAttachmentLoading}
              className="mb-1 h-8 w-8 text-muted-foreground hover:text-foreground shrink-0"
              title="Attach files"
              aria-label="Attach files"
            >
              <Paperclip className="h-4 w-4" />
            </Button>
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => {
                setInput(e.target.value);
                onInputChange(
                  e.target.value,
                  e.target.selectionStart ?? e.target.value.length,
                );
              }}
              placeholder={
                dragState === 'valid'
                  ? 'Drop files here...'
                  : dragState === 'invalid'
                    ? 'Unsupported file!'
                    : isAttachmentLoading
                      ? 'Uploading...'
                      : `Message ${assistant.name}...`
              }
              className="flex-1 bg-transparent border-none focus:ring-0 resize-none max-h-32 min-h-11 py-3 px-2"
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSubmit(e);
                }
              }}
              disabled={isSubmitting || isAttachmentLoading}
            />
            <Button
              type="submit"
              size="icon"
              disabled={
                (!input.trim() && pendingFiles.length === 0) ||
                isSubmitting ||
                isAttachmentLoading
              }
              className="mb-1"
            >
              {isSubmitting || isAttachmentLoading ? (
                <Loader2 className="animate-spin" />
              ) : (
                <Send className="w-4 h-4" />
              )}
            </Button>
          </form>
        </div>
      </div>
    </div>
  );
}

export default function AgentDraftChatView() {
  return <DraftChatInner />;
}
