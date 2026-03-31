import { useAssistantContext } from '@/context/AssistantContext';
import { useCallback, useMemo, useState } from 'react';
import { Assistant } from '../../models/chat';
import { Badge, Button } from '@/components/ui';
import { EditorProvider } from '@/context/EditorContext';
import { toast } from 'sonner';
import AssistantEditor from './AssistantEditor';
import { useTranslation } from 'react-i18next';
import {
  enforceRuntimeBuiltinAliases,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '@/lib/assistant/runtime-builtins';

import {
  ChevronDown,
  ChevronUp,
  Bot,
  Trash2,
  Edit,
  Calendar,
  Puzzle,
  Square,
} from 'lucide-react';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';

let dateFormatter: Intl.DateTimeFormat | null = null;
function getDateFormatter() {
  if (!dateFormatter) {
    dateFormatter = new Intl.DateTimeFormat();
  }
  return dateFormatter;
}

interface AssistantCardProps {
  assistant: Assistant;
  isExpanded: boolean;
  onToggle: () => void;
  builtinToolsMap?: Record<string, string>;
  mcpServersMap?: Record<string, string>;
}

export default function AssistantCard({
  assistant,
  isExpanded,
  onToggle,
  builtinToolsMap,
  mcpServersMap,
}: AssistantCardProps) {
  const { deleteAssistant, saveAssistant: upsertAssistant } =
    useAssistantContext();
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [edit, setEdit] = useState<boolean>(false);
  const { t } = useTranslation('common');
  const logger = getLogger('AssistantCard');

  const handleEditComplete = useCallback(
    async (assistant: Assistant) => {
      upsertAssistant(assistant);
    },
    [upsertAssistant],
  );

  const handleDeleteClick = useCallback(() => {
    if (assistant.deletionProtected === true) {
      toast.error(t('assistant.card.deleteBlocked'));
      return;
    }
    setShowDeleteConfirm(true);
  }, [assistant.deletionProtected, t]);

  const handleDeleteConfirm = useCallback(async () => {
    setIsDeleting(true);
    try {
      if (assistant.id) {
        await deleteAssistant(assistant.id);
        logger.info('Assistant deleted', { assistantId: assistant.id });
      }
    } catch (error) {
      logger.error('Failed to delete assistant', error);
    } finally {
      setIsDeleting(false);
      setShowDeleteConfirm(false);
    }
  }, [assistant.id, deleteAssistant, logger]);

  const handleDeleteCancel = useCallback(() => {
    setShowDeleteConfirm(false);
  }, []);

  const effectiveBuiltinAliases = useMemo(
    () => enforceRuntimeBuiltinAliases(assistant.allowedBuiltInServiceAliases),
    [assistant.allowedBuiltInServiceAliases],
  );

  const enabledOptionalAliases = useMemo(
    () =>
      effectiveBuiltinAliases.filter((alias) =>
        OPTIONAL_BUILTIN_SERVICE_ALIASES.includes(
          alias as (typeof OPTIONAL_BUILTIN_SERVICE_ALIASES)[number],
        ),
      ),
    [effectiveBuiltinAliases],
  );

  return (
    <EditorProvider initialValue={assistant} onFinalize={handleEditComplete}>
      <div
        className={cn(
          'group border rounded-[1.5rem] p-5 transition-all duration-300 relative overflow-hidden bg-background/50 backdrop-blur-sm',
          isExpanded
            ? 'ring-1 ring-primary/20 bg-background/80 shadow-xl'
            : 'hover:border-primary/40 hover:shadow-lg hover:bg-background border-border/50',
        )}
      >
        {/* Identity Row */}
        <div className="flex justify-between items-start mb-4 relative z-10">
          <div className="flex items-center gap-4 flex-1">
            <div
              className={cn(
                'w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300',
                isExpanded
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-primary/10 text-primary group-hover:scale-110',
              )}
            >
              <Bot size={20} />
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-0.5">
                <h3 className="text-lg font-bold tracking-tight">
                  {assistant.name}
                </h3>
                {assistant.deletionProtected === true && (
                  <Badge
                    variant="destructive"
                    className="text-[9px] uppercase font-bold tracking-widest h-4 px-1.5 font-sans"
                  >
                    {t('assistant.card.protected')}
                  </Badge>
                )}
              </div>
              <p className="text-xs text-muted-foreground/70 font-sans line-clamp-1 italic">
                {isExpanded
                  ? 'Full Configuration'
                  : assistant.description || assistant.systemPrompt}
              </p>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 rounded-full hover:bg-primary/5 transition-colors"
            onClick={onToggle}
            aria-label={
              isExpanded
                ? t('assistant.card.collapse', 'Collapse configuration')
                : t('assistant.card.expand', 'Expand configuration')
            }
          >
            {isExpanded ? (
              <ChevronUp className="h-4 w-4" />
            ) : (
              <ChevronDown className="h-4 w-4 text-muted-foreground" />
            )}
          </Button>
        </div>

        {/* Detailed Content */}
        {isExpanded && (
          <div className="space-y-6 mb-6 relative z-10 animate-in fade-in slide-in-from-top-2 duration-300">
            <div className="space-y-2">
              <div className="text-[10px] uppercase font-bold tracking-widest text-muted-foreground/60 font-sans flex items-center gap-1.5">
                <Square size={10} />
                {t('assistant.systemPromptLabel')}
              </div>
              <div className="bg-muted/30 border border-border/50 rounded-xl p-4 font-mono text-sm leading-relaxed text-foreground/90 whitespace-pre-wrap">
                {assistant.systemPrompt}
              </div>
            </div>

            {assistant.description && (
              <div className="space-y-2">
                <div className="text-[10px] uppercase font-bold tracking-widest text-muted-foreground/60 font-sans flex items-center gap-1.5">
                  <Square size={10} />
                  {t('assistant.card.description')}
                </div>
                <p className="text-sm text-muted-foreground font-sans leading-relaxed px-1">
                  {assistant.description}
                </p>
              </div>
            )}

            <div className="flex items-center gap-6 pt-2 border-t border-border/40">
              <div className="flex items-center gap-2 text-[10px] text-muted-foreground/60 uppercase tracking-widest font-sans">
                <Calendar size={12} />
                <span>
                  {t('assistant.card.created')}:{' '}
                  {getDateFormatter().format(new Date(assistant.createdAt))}
                </span>
              </div>
              <div className="flex items-center gap-2 text-[10px] text-muted-foreground/60 uppercase tracking-widest font-sans">
                <Edit size={12} />
                <span>
                  {t('assistant.card.updated')}:{' '}
                  {getDateFormatter().format(new Date(assistant.updatedAt))}
                </span>
              </div>
            </div>
          </div>
        )}

        {/* Tools Section */}
        <div className="flex flex-wrap gap-1.5 mb-5 relative z-10">
          {/* External MCP Servers - Blue/Primary Badges */}
          {assistant.mcpServerIds?.map((serverId) => {
            const serverName = mcpServersMap?.[serverId] ?? serverId;
            return (
              <Badge
                key={serverId}
                variant="outline"
                className="bg-primary/5 text-primary/80 border-primary/20 text-[10px] font-medium font-sans flex items-center gap-1 py-0.5"
              >
                <Puzzle size={10} />
                {serverName}
              </Badge>
            );
          })}

          {/* Built-in Tools - Green/Success Badges */}
          <Badge
            variant="secondary"
            className="bg-success/10 text-success/80 border-transparent text-[10px] font-medium font-sans flex items-center gap-1 py-0.5"
          >
            <Square size={10} />
            {t('assistant.card.coreBuiltin')}
          </Badge>
          {enabledOptionalAliases.map((alias) => (
            <Badge
              key={alias}
              variant="secondary"
              className="bg-success/10 text-success/80 border-transparent text-[10px] font-medium font-sans flex items-center gap-1 py-0.5"
            >
              <Square size={10} />
              {builtinToolsMap?.[alias] || alias}
            </Badge>
          ))}
        </div>

        {/* Action Buttons */}
        <div className="flex flex-wrap gap-2 relative z-10">
          {!showDeleteConfirm ? (
            <>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setEdit(true)}
                className="rounded-lg px-4 font-bold font-sans text-xs bg-muted/50 hover:bg-muted"
              >
                <Edit size={14} className="mr-1.5 opacity-70" />
                {t('assistant.card.edit')}
              </Button>

              <Button
                size="sm"
                variant="ghost"
                onClick={handleDeleteClick}
                disabled={isDeleting}
                className="rounded-lg px-4 font-bold font-sans text-xs text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                title={
                  assistant.deletionProtected === true
                    ? t('assistant.card.deleteBlocked')
                    : t('assistant.card.delete')
                }
              >
                <Trash2 size={14} className="mr-1.5 opacity-70" />
                {t('assistant.card.delete')}
              </Button>
            </>
          ) : (
            <div className="flex gap-2 w-full animate-in zoom-in-95 duration-200">
              <Button
                size="sm"
                variant="destructive"
                onClick={handleDeleteConfirm}
                disabled={isDeleting}
                className="flex-1 rounded-lg font-bold font-sans"
              >
                {isDeleting
                  ? t('assistant.card.deleting')
                  : t('assistant.card.confirmDelete')}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleDeleteCancel}
                disabled={isDeleting}
                className="rounded-lg font-bold font-sans"
              >
                {t('assistant.card.cancel')}
              </Button>
            </div>
          )}
        </div>
      </div>
      <AssistantEditor.Dialog open={edit} onOpenChange={setEdit} />
    </EditorProvider>
  );
}
