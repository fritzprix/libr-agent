import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

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
      <div className="flex items-center justify-between px-6 py-4 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex items-center gap-3">
          <div className="flex flex-col">
            <span className="font-semibold text-lg">{assistant.name}</span>
            <span className="text-xs text-muted-foreground">
              {t('agent.draft.newSession')}
            </span>
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
              <div className="absolute top-0 right-0 p-2 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                <button
                  type="button"
                  onClick={() => setWorkspaceOverride(null)}
                  className="bg-background/80 hover:bg-background rounded-full p-1 shadow-sm border focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                  title="Remove workspace override"
                  aria-label="Remove workspace override"
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
              <p>{t('agent.draft.basicToolsDescription')}</p>
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
            <Bot size={10} />
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
                  className="shrink-0 text-muted-foreground hover:text-foreground ml-0.5 rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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
              aria-label={t('agent.draft.send', 'Send message')}
              title={t('agent.draft.send', 'Send message')}
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
