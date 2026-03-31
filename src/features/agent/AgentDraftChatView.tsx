import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useLayoutEffect } from 'react';

import {
  Button,
  Badge,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
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
  Puzzle,
  Paperclip,
  X,
} from 'lucide-react';
import { useSettings } from '@/context/SettingsContext';
import { cn } from '@/lib/utils';
import {
  enforceRuntimeBuiltinAliases,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '@/lib/assistant/runtime-builtins';
import { InputTokenDropdown } from './components/InputTokenDropdown';
import { useAgentDraftChat } from './hooks/useAgentDraftChat';

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

const textareaStyle = {
  msOverflowStyle: 'none',
  scrollbarWidth: 'none',
} as const;

function DraftChatInner() {
  const { t } = useTranslation();
  const { value: settings } = useSettings();

  const {
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
  } = useAgentDraftChat();

  // Auto-resize textarea - Mirrored from AgentChatInput
  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      const maxHeightPx = 128; // max-h-32 (8rem)
      const nextHeight = Math.min(textarea.scrollHeight, maxHeightPx);
      textarea.style.height = `${nextHeight}px`;
    }
  }, [input]);

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

  // Synchronized placeholder logic from AgentChatInput
  const inputPlaceholder = (() => {
    if (dragState !== 'none') {
      return dragState === 'valid'
        ? t('agent.input.placeholderDropValid')
        : t('agent.input.placeholderDropInvalid');
    }
    if (isAttachmentLoading) return t('agent.input.placeholderUploading');
    return t('agent.input.placeholderDefault');
  })();

  const hasAttachedFiles = pendingFiles.length > 0;

  return (
    <div className="flex h-full w-full overflow-hidden rounded-2xl border border-border/50 bg-background font-sans shadow-[0_18px_48px_-28px_rgba(0,0,0,0.35)]">
      {/* Workspace Side Panel (Placeholder/Static for Draft) */}
      {workspaceOverride && (
        <div className="w-80 h-full border-r bg-background/95 backdrop-blur flex flex-col animate-in slide-in-from-left duration-300">
          <div className="px-4 py-3 border-b flex items-center justify-between">
            <div className="flex items-center gap-2 text-sm font-medium">
              <FolderOpen className="w-4 h-4 text-primary" />
              <span>Workspace</span>
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 text-muted-foreground hover:text-foreground"
              onClick={() => setWorkspaceOverride(null)}
              aria-label={t('common:close', 'Close')}
            >
              <X className="w-3 h-3" />
            </Button>
          </div>
          <div className="p-4 flex-1 overflow-auto space-y-4">
            <div>
              <div className="text-[10px] text-primary font-bold uppercase tracking-wider mb-2">
                {t(
                  'agent.draft.workspaceOverrideActive',
                  'Workspace Override Active',
                )}
              </div>
              <div className="bg-muted/50 p-2 rounded-md font-mono text-[10px] break-all border border-border/50">
                {workspaceOverride}
              </div>
            </div>
            <div className="p-6 border border-dashed rounded-xl text-center flex flex-col items-center gap-3 bg-muted/20">
              <Loader2 className="w-5 h-5 text-muted-foreground/40 animate-spin" />
              <p className="text-xs text-muted-foreground leading-relaxed">
                {t(
                  'agent.draft.filesWillBeListedAfterStart',
                  'Files will be listed after session starts',
                )}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col min-h-0 min-w-0 relative bg-background">
        {/* Session header - aligned with the shared agent session header style */}
        <div className="px-4 py-3 flex items-center justify-between border-b flex-shrink-0 bg-background/95 backdrop-blur z-20">
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-muted-foreground uppercase font-sans font-bold tracking-widest">
              Assistant:
            </span>
            <span className="text-xs font-semibold text-primary">
              [{assistant.name}]
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-muted-foreground uppercase font-sans font-bold tracking-widest">
              Session:
            </span>
            <span className="text-xs truncate max-w-xs italic text-muted-foreground/80">
              {t('agent.draft.newSession', 'New Session')} (Agent)
            </span>
          </div>
        </div>

        {/* Scrollable Content Area */}
        <div className="flex-1 overflow-y-auto no-scrollbar relative">
          <div
            ref={profileAreaRef}
            className={cn(
              'min-h-full p-8 pb-32 flex flex-col items-center justify-center text-center gap-10 transition-all duration-500',
              profileDragState === 'valid' &&
                'bg-primary/5 ring-2 ring-primary/30 ring-inset',
              profileDragState === 'invalid' &&
                'bg-destructive/10 ring-2 ring-destructive/30 ring-inset',
            )}
          >
            {/* Identity Section */}
            <div className="flex flex-col items-center space-y-6 animate-in fade-in zoom-in duration-700">
              <div className="w-24 h-24 bg-primary/10 rounded-[1.5rem] flex items-center justify-center shadow-xl ring-1 ring-primary/20 transition-transform hover:scale-105 duration-300">
                <Bot className="w-12 h-12 text-primary" />
              </div>
              <div className="space-y-3 max-w-xl">
                <h1 className="text-4xl font-bold tracking-tight text-foreground">
                  {assistant.name}
                </h1>
                {assistant.description && (
                  <p className="text-muted-foreground text-base leading-relaxed max-w-md mx-auto font-sans">
                    {assistant.description}
                  </p>
                )}
              </div>
            </div>

            {/* Workspace State / Drop Hint */}
            {!workspaceOverride && (
              <div
                className={cn(
                  'text-[11px] uppercase tracking-wider font-sans transition-all duration-300 border border-dashed rounded-full px-6 py-2 bg-background/50 backdrop-blur-sm shadow-sm',
                  profileDragState === 'valid'
                    ? 'opacity-100 font-bold border-primary/50 text-primary bg-primary/5 scale-105'
                    : 'opacity-40 text-muted-foreground border-border hover:opacity-100',
                )}
              >
                {t(
                  'agent.workspace.dropFolderHint',
                  'Drop folder to set workspace context',
                )}
              </div>
            )}

            {/* Capabilities Grid */}
            <div className="flex flex-wrap gap-2 justify-center max-w-2xl animate-in fade-in slide-in-from-bottom-4 duration-700 delay-150">
              <Tooltip delayDuration={300}>
                <TooltipTrigger asChild>
                  <div className="cursor-help">
                    <Badge
                      variant="secondary"
                      className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans bg-muted/40 hover:bg-muted transition-colors border border-transparent hover:border-border/50"
                    >
                      <Square size={12} className="text-primary/70" />
                      {t('agent.draft.basicTools', 'Core Abilities')}
                    </Badge>
                  </div>
                </TooltipTrigger>
                <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-xl border">
                  <p className="text-xs">
                    {t(
                      'agent.draft.basicToolsDescription',
                      'Standard capabilities available to all agents.',
                    )}
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
                          className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans bg-muted/40 hover:bg-muted transition-colors border border-transparent hover:border-border/50"
                        >
                          <Icon size={12} className="text-primary/70" />
                          {label}
                        </Badge>
                      </div>
                    </TooltipTrigger>
                    {info?.metadata.description && (
                      <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-xl border">
                        <p className="text-xs">{info.metadata.description}</p>
                      </TooltipContent>
                    )}
                  </Tooltip>
                );
              })}

              {assistant.mcpServerIds?.map((serverId) => {
                const serverConfig = mcpServers.find((s) => s.id === serverId);
                const label = serverConfig?.name || serverId;
                return (
                  <Badge
                    key={serverId}
                    variant="outline"
                    className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans border-dashed border-primary/30 text-primary/80 bg-primary/5"
                  >
                    <Puzzle size={12} />
                    {label}
                  </Badge>
                );
              })}

              <Link to="/assistants">
                <Tooltip delayDuration={300}>
                  <TooltipTrigger asChild>
                    <Badge
                      variant="outline"
                      className="text-[11px] text-muted-foreground/60 border-dashed font-sans font-normal cursor-pointer hover:opacity-100 hover:bg-muted hover:text-foreground transition-all px-3 py-1.5"
                    >
                      {t('agent.draft.addTools', 'Customize Abilities')}
                    </Badge>
                  </TooltipTrigger>
                  <TooltipContent className="mb-1 bg-popover text-popover-foreground border shadow-xl">
                    <p className="text-xs">
                      {t(
                        'agent.draft.addMoreCapabilities',
                        'Manage tools and MCP servers for this agent.',
                      )}
                    </p>
                  </TooltipContent>
                </Tooltip>
              </Link>
            </div>

            {/* Configuration Section */}
            <div className="flex flex-col items-center gap-5 mt-4 pt-8 border-t border-border/40 w-full max-w-md animate-in fade-in duration-1000">
              <AgentModelPicker
                currentModel={
                  overrideModel || settings?.preferredModel?.model || 'gpt-4'
                }
                currentProvider={
                  overrideProvider ||
                  settings?.preferredModel?.provider ||
                  'openai'
                }
                onConfigUpdate={(model, provider) => {
                  setOverrideModel(model);
                  setOverrideProvider(provider);
                }}
                className="w-full max-w-xs shadow-sm"
              />
              <div className="flex items-center gap-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground/30 font-bold font-sans">
                <div className="h-px w-6 bg-border/40" />
                <Bot size={14} className="opacity-50" />
                {t('agent.draft.localContext', 'Local Context Ready')}
                <div className="h-px w-6 bg-border/40" />
              </div>
            </div>
          </div>
        </div>

        {/* Floating Input Area - Precision Aligned with AgentChatInput */}
        <div className="absolute bottom-0 left-0 right-0 z-10 pointer-events-none">
          <div className="h-32 bg-gradient-to-t from-background via-background/60 to-transparent w-full" />
          <div className="p-4 pt-0">
            <div className="w-full max-w-5xl mx-auto pointer-events-auto">
              {/* Attached Files List - Mirrored from AgentChatAttachedFiles */}
              {hasAttachedFiles && (
                <div className="px-4 py-3 bg-background/60 backdrop-blur-md rounded-t-xl border-x border-t border-border/50 animate-in slide-in-from-bottom-2 duration-300">
                  <div className="text-[10px] mb-2 flex items-center gap-1.5 font-bold text-muted-foreground font-sans uppercase tracking-widest">
                    <Paperclip className="w-3.5 h-3.5" />
                    <span>
                      {t('agent.draft.attachedFiles', 'Attached Files')}:
                    </span>
                  </div>
                  <ul className="flex flex-wrap gap-2">
                    {pendingFiles.map((file, index) => (
                      <li
                        key={`${file.name}-${index}`}
                        className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg border border-border bg-background/50 shadow-sm transition-all hover:border-primary/30"
                      >
                        <span className="text-xs font-medium font-sans truncate max-w-[200px]">
                          {file.name}
                        </span>
                        <button
                          type="button"
                          onClick={() => handleFileRemove(index)}
                          className="text-muted-foreground hover:text-destructive transition-colors focus:outline-none"
                          aria-label={t(
                            'agent.draft.removeFile',
                            'Remove file',
                          )}
                        >
                          <X className="w-3.5 h-3.5" />
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {/* Input Form - Exact match for AgentChatInput formClassName */}
              <div className="relative group">
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
                        const newValue = onTypeSelect(
                          typeName,
                          input,
                          cursorPos,
                        );
                        setInput(newValue);
                        requestAnimationFrame(() => {
                          if (textareaRef.current) {
                            const pos =
                              newValue.length - (input.length - cursorPos);
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
                        requestAnimationFrame(() =>
                          textareaRef.current?.focus(),
                        );
                      }}
                      onDismiss={onDismiss}
                    />
                  )}

                <form
                  ref={formRef}
                  onSubmit={handleSubmit}
                  className={cn(
                    'flex items-end gap-2 bg-background/60 backdrop-blur-md p-3 border border-border/50 shadow-2xl focus-within:ring-1 focus-within:ring-primary/20 transition-all duration-300',
                    hasAttachedFiles ? 'rounded-b-xl border-t-0' : 'rounded-xl',
                    dragState === 'valid' &&
                      'bg-success/5 border-success/50 shadow-success/10',
                    dragState === 'invalid' &&
                      'bg-destructive/5 border-destructive/50 shadow-destructive/10',
                  )}
                >
                  <input
                    ref={fileInputRef}
                    type="file"
                    multiple
                    onChange={handleFileAdd}
                    className="hidden"
                  />

                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => fileInputRef.current?.click()}
                        disabled={isSubmitting || isAttachmentLoading}
                        className="mb-1 h-8 w-8 text-muted-foreground hover:text-primary hover:bg-primary/5 shrink-0 transition-colors"
                        aria-label={t(
                          'agent.draft.attachFiles',
                          'Attach files',
                        )}
                      >
                        <Paperclip className="h-4 w-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('agent.draft.attachFiles', 'Attach files')}
                    </TooltipContent>
                  </Tooltip>

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
                    placeholder={inputPlaceholder}
                    rows={1}
                    autoComplete="off"
                    spellCheck="false"
                    style={textareaStyle}
                    className="flex-1 resize-none bg-transparent outline-none border-none py-3 px-2 text-sm leading-relaxed max-h-32 min-h-[44px] overflow-y-auto transition-colors"
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault();
                        if (
                          !isAttachmentLoading &&
                          (input.trim() || hasAttachedFiles)
                        ) {
                          handleSubmit(e);
                        }
                      }
                    }}
                    disabled={isSubmitting || isAttachmentLoading}
                  />

                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="submit"
                        disabled={
                          (!input.trim() && !hasAttachedFiles) ||
                          isSubmitting ||
                          isAttachmentLoading
                        }
                        size="icon"
                        className="mb-1 shrink-0 shadow-lg transition-all active:scale-95"
                        aria-label={t(
                          'agent.draft.sendMessage',
                          'Send message',
                        )}
                      >
                        {isSubmitting || isAttachmentLoading ? (
                          <Loader2 className="animate-spin h-4 w-4" />
                        ) : (
                          <Send className="h-4 w-4" />
                        )}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('agent.draft.sendMessage', 'Send message')}
                    </TooltipContent>
                  </Tooltip>
                </form>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function AgentDraftChatView() {
  return <DraftChatInner />;
}
