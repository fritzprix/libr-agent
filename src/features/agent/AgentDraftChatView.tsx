import { useTranslation } from 'react-i18next';
import { type CSSProperties } from 'react';

import {
  Button,
  FileAttachment,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { AgentDraftWorkspacePreviewPanel } from './components/AgentDraftWorkspacePreviewPanel';
import { AgentAttachedFilesBar } from './components/AgentAttachedFilesBar';
import AgentSessionHeader from './components/AgentSessionHeader';
import { DraftCapabilitiesSection } from './components/DraftCapabilitiesSection';
import { Send, Loader2, Bot } from 'lucide-react';
import { useSettings } from '@/context/SettingsContext';
import { cn } from '@/lib/utils';
import { InputTokenDropdown } from './components/InputTokenDropdown';
import { useAgentDraftChat } from './hooks/useAgentDraftChat';
import { useWorkspaceFiles } from './hooks/useWorkspaceFiles';
import { AGENT_ATTACHMENT_PICKER_ACCEPT } from './lib/attachment-picker';
import { useTextareaAutosize } from '@/hooks/useTextareaAutosize';

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

  const fileQuery =
    stage.kind === 'typing-arg' && stage.typeName === 'file'
      ? stage.query
      : null;
  const workspaceFileResults = useWorkspaceFiles(
    undefined,
    fileQuery,
    workspaceOverride,
  );

  useTextareaAutosize({
    textareaRef,
    value: input,
    maxHeight: 128,
  });

  if (isLoadingAssistant) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (!assistant) return null;

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
      {/* Workspace Side Panel */}
      {workspaceOverride && (
        <AgentDraftWorkspacePreviewPanel
          workspacePath={workspaceOverride}
          onClear={() => setWorkspaceOverride(null)}
        />
      )}

      {/* Main Chat Area */}
      <div
        className="flex min-h-0 min-w-0 flex-1 flex-col"
        style={
          {
            '--agent-chat-composer-overlap': '64px',
          } as CSSProperties
        }
      >
        <AgentSessionHeader
          assistantName={`[${assistant.name}]`}
          assistantNameClassName="text-xs font-semibold text-primary"
          sessionName={t('agent.draft.newSession', 'New Session')}
          sessionNameClassName="max-w-xs truncate text-xs italic text-muted-foreground/80"
        />

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

            <DraftCapabilitiesSection
              assistant={assistant}
              builtinServices={builtinServices}
              mcpServers={mcpServers}
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
            />
          </div>
        </div>

        <div className="relative shrink-0 px-4 pb-4">
          <div
            aria-hidden="true"
            style={{ height: 'var(--agent-chat-composer-overlap, 64px)' }}
          />
          <div
            className="relative z-10"
            style={{
              marginTop: 'calc(var(--agent-chat-composer-overlap, 64px) * -1)',
            }}
          >
            <div className="pointer-events-none absolute inset-x-0 -top-12 h-32 bg-gradient-to-t from-background/80 via-background/28 to-transparent" />
            <div className="pointer-events-auto mx-auto w-full max-w-5xl">
              {/* Attached Files List - Mirrored from AgentChatAttachedFiles */}
              {hasAttachedFiles && (
                <AgentAttachedFilesBar
                  files={pendingFiles.map((file, index) => ({
                    id: `${file.name}-${index}`,
                    name: file.name,
                    onRemove: () => handleFileRemove(index),
                  }))}
                  title={t('agent.draft.attachedFiles', 'Attached Files:')}
                />
              )}

              {/* Input Form - Exact match for AgentChatInput formClassName */}
              <div className="relative group">
                {stage.kind !== 'idle' &&
                  (typeResults.length > 0 ||
                    skillResults.length > 0 ||
                    workspaceFileResults.length > 0) && (
                    <InputTokenDropdown
                      mode={
                        stage.kind === 'typing-type'
                          ? { kind: 'types', items: typeResults }
                          : stage.typeName === 'file'
                            ? { kind: 'files', items: workspaceFileResults }
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
                  <FileAttachment
                    files={pendingFiles.map((file) => ({
                      name: file.name,
                      content: '',
                    }))}
                    onAdd={handleFileAdd}
                    compact={true}
                    disabled={isSubmitting || isAttachmentLoading}
                    showFileCount={false}
                    accept={AGENT_ATTACHMENT_PICKER_ACCEPT}
                    buttonClassName="mb-1 h-8 w-8 hover:text-primary hover:bg-primary/5 transition-colors"
                  />

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
                      if (
                        stage.kind !== 'idle' &&
                        (typeResults.length > 0 ||
                          skillResults.length > 0 ||
                          workspaceFileResults.length > 0) &&
                        [
                          'ArrowUp',
                          'ArrowDown',
                          'Enter',
                          'Tab',
                          'Escape',
                        ].includes(e.key)
                      ) {
                        return;
                      }

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
                      <span
                        tabIndex={
                          (!input.trim() && !hasAttachedFiles) ||
                          isSubmitting ||
                          isAttachmentLoading
                            ? 0
                            : undefined
                        }
                        className={cn(
                          'inline-block rounded-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none mb-1 shrink-0',
                          ((!input.trim() && !hasAttachedFiles) ||
                            isSubmitting ||
                            isAttachmentLoading) &&
                            'cursor-not-allowed',
                        )}
                        aria-label={
                          (!input.trim() && !hasAttachedFiles) ||
                          isSubmitting ||
                          isAttachmentLoading
                            ? t('agent.input.sendAriaLabel', 'Send message')
                            : undefined
                        }
                        aria-disabled={
                          (!input.trim() && !hasAttachedFiles) ||
                          isSubmitting ||
                          isAttachmentLoading
                            ? true
                            : undefined
                        }
                        role={
                          (!input.trim() && !hasAttachedFiles) ||
                          isSubmitting ||
                          isAttachmentLoading
                            ? 'button'
                            : undefined
                        }
                      >
                        <Button
                          type="submit"
                          disabled={
                            (!input.trim() && !hasAttachedFiles) ||
                            isSubmitting ||
                            isAttachmentLoading
                          }
                          size="icon"
                          className={cn(
                            'shadow-lg transition-all active:scale-95',
                            ((!input.trim() && !hasAttachedFiles) ||
                              isSubmitting ||
                              isAttachmentLoading) &&
                              'pointer-events-none',
                          )}
                          aria-label={t(
                            'agent.input.sendAriaLabel',
                            'Send message',
                          )}
                        >
                          {isSubmitting || isAttachmentLoading ? (
                            <Loader2 className="animate-spin h-4 w-4" />
                          ) : (
                            <Send className="h-4 w-4" />
                          )}
                        </Button>
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('agent.input.sendTooltip', 'Send')}
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
