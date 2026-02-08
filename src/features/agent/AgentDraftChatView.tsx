import { useState, useCallback, useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { createId } from '@paralleldrive/cuid2';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { Button, Badge } from '@/components/ui';
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
} from 'lucide-react';
import type { Assistant, Message } from '@/models/chat';
import { parseAssistant } from '@/models/validation';
import { useSettings } from '@/context/SettingsContext';

const logger = getLogger('AgentDraftChatView');

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

  useEffect(() => {
    const loadMetadata = async () => {
      try {
        const [services, servers] = await Promise.all([
          invoke<BuiltinServerInfo[]>(
            'list_available_builtin_server_definitions',
          ),
          invoke<MCPServerDto[]>('list_mcp_server_configs'),
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
        const rawData = await invoke('get_assistant', {
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
      if (!input.trim() || !assistant || isSubmitting) return;

      setIsSubmitting(true);
      const newSessionId = createId();
      const now = new Date();
      // Temporary state for submission progress
      let unlisten: (() => void) | undefined;
      let toastId: string | number | undefined;

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

        // Prepare message
        const initialMessage: Message = {
          id: createId(),
          sessionId: newSessionId,
          threadId: newSessionId,
          role: 'user',
          content: [{ type: 'text', text: input.trim() }],
          createdAt: now,
          updatedAt: now,
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
          allowedBuiltInServiceAliases: assistant.allowedBuiltInServiceAliases,
          temperature: 0.7,
          maxTokens: settings?.advanced?.defaultMaxOutputTokens ?? 8192,
        };

        if (!toastId) toastId = toast.loading('Creating session...');

        // Atomic Create + Send
        await invoke('agent_create_session_with_initial_message', {
          request: {
            sessionId: newSessionId,
            name: shortName,
            model: overrideModel || settings?.preferredModel?.model || 'gpt-4',
            provider:
              overrideProvider ||
              settings?.preferredModel?.provider ||
              'openai',
            agentConfig,
            message: rustMessage,
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
            <span className="text-xs text-muted-foreground">New Session</span>
          </div>
        </div>
      </div>

      {/* Assistant Profile Card */}
      <div className="flex-1 p-8 flex flex-col items-center justify-center text-center gap-6 overflow-y-auto no-scrollbar">
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

        {/* Capabilities Grid */}
        <div className="flex flex-wrap gap-2 justify-center max-w-2xl mt-2">
          {/* Built-in Tools: If allowedBuiltInServiceAliases is undefined/null, it means ALL are allowed */}
          {(
            assistant.allowedBuiltInServiceAliases ||
            builtinServices.map((s) => s.name)
          )?.map((alias) => {
            const info = builtinServices.find((s) => s.name === alias);
            const label = info?.metadata.displayName || alias;
            const Icon = getIconForService(info?.metadata.icon);

            return (
              <Badge
                key={alias}
                variant="secondary"
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-normal"
                title={info?.metadata.description} // Tooltip showing description
              >
                <Icon size={12} className="opacity-70" />
                {label}
              </Badge>
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

          {/* Fallback if list is EXPLICITLY empty (not undefined, which means all) */}
          {assistant.allowedBuiltInServiceAliases &&
            assistant.allowedBuiltInServiceAliases.length === 0 &&
            (!assistant.mcpServerIds ||
              assistant.mcpServerIds.length === 0) && (
              <Badge
                variant="outline"
                className="text-xs text-muted-foreground opacity-50"
              >
                No specific tools enabled
              </Badge>
            )}
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
            Local Context
          </div>
        </div>
      </div>

      {/* Simplified Input Area */}
      <div className="p-4 border-t">
        <form
          onSubmit={handleSubmit}
          className="flex items-end gap-2 bg-muted/30 p-2 rounded-lg border focus-within:ring-1 focus-within:ring-primary/20"
        >
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={`Message ${assistant.name}...`}
            className="flex-1 bg-transparent border-none focus:ring-0 resize-none max-h-32 min-h-11 py-3 px-2"
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSubmit(e);
              }
            }}
            disabled={isSubmitting}
          />
          <Button
            type="submit"
            size="icon"
            disabled={!input.trim() || isSubmitting}
            className="mb-1"
          >
            {isSubmitting ? (
              <Loader2 className="animate-spin" />
            ) : (
              <Send className="w-4 h-4" />
            )}
          </Button>
        </form>
      </div>
    </div>
  );
}

export default function AgentDraftChatView() {
  return <DraftChatInner />;
}
