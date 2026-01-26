import { useAssistantContext } from '@/context/AssistantContext';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useCallback, useState } from 'react';
import { Assistant } from '../../models/chat';
import { Badge, Button } from '@/components/ui';
import { EditorProvider } from '@/context/EditorContext';
import AssistantEditor from './AssistantEditor';
import { useTranslation } from 'react-i18next';

import { ChevronDown, ChevronUp } from 'lucide-react';
import { getLogger } from '@/lib/logger';

interface AssistantCardProps {
  assistant: Assistant;
  isExpanded: boolean;
  onToggle: () => void;
  builtinToolsMap?: Record<string, string>;
}

export default function AssistantCard({
  assistant,
  isExpanded,
  onToggle,
  builtinToolsMap,
}: AssistantCardProps) {
  const { deleteAssistant, saveAssistant: upsertAssistant } =
    useAssistantContext();
  const { serversById } = useMCPServer();
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
      alert(t('assistant.card.deleteBlocked'));
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
  return (
    <EditorProvider initialValue={assistant} onFinalize={handleEditComplete}>
      <div className="border rounded p-3 transition-colors border-muted hover:border-accent">
        <div className="flex justify-between items-start mb-2">
          <div className="flex items-center gap-2 flex-1">
            <h3 className="text-primary font-medium">{assistant.name}</h3>
            {assistant.deletionProtected === true && (
              <Badge variant="destructive">
                {t('assistant.card.protected')}
              </Badge>
            )}
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={onToggle}
          >
            {isExpanded ? (
              <ChevronUp className="h-4 w-4" />
            ) : (
              <ChevronDown className="h-4 w-4" />
            )}
          </Button>
        </div>

        {isExpanded ? (
          <div className="space-y-4 mb-3">
            <div>
              <p className="text-sm font-medium mb-1">
                {t('assistant.systemPromptLabel')}
              </p>
              <p className="text-muted-foreground text-sm whitespace-pre-wrap">
                {assistant.systemPrompt}
              </p>
            </div>

            {assistant.description && (
              <div>
                <p className="text-sm font-medium mb-1">
                  {t('assistant.card.description')}
                </p>
                <p className="text-muted-foreground text-sm">
                  {assistant.description}
                </p>
              </div>
            )}

            <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
              <div>
                <span className="font-medium">
                  {t('assistant.card.created')}:
                </span>{' '}
                {new Date(assistant.createdAt).toLocaleDateString()}
              </div>
              <div>
                <span className="font-medium">
                  {t('assistant.card.updated')}:
                </span>{' '}
                {new Date(assistant.updatedAt).toLocaleDateString()}
              </div>
            </div>
          </div>
        ) : (
          <p className="text-muted-foreground text-sm mb-3 line-clamp-2">
            {assistant.systemPrompt}
          </p>
        )}

        <div className="flex flex-wrap gap-1 mb-2">
          {/* External MCP Servers - Blue Badges */}
          {assistant.mcpServerIds?.map((serverId) => {
            const serverMeta = serversById?.[serverId];
            const serverName = serverMeta?.name ?? serverId;
            return (
              <Badge
                key={serverId}
                variant="outline"
                className="bg-primary/10 text-primary border-primary/20"
              >
                {serverName}
              </Badge>
            );
          })}

          {/* Built-in Tools - Green Badges */}
          {assistant.allowedBuiltInServiceAliases === undefined ? (
            <Badge
              variant="secondary"
              className="bg-success/10 text-success"
            >
              {t('assistant.card.allBuiltin')}
            </Badge>
          ) : (
            assistant.allowedBuiltInServiceAliases.map((alias) => (
              <Badge
                key={alias}
                variant="secondary"
                className="bg-success/10 text-success"
              >
                {builtinToolsMap?.[alias] || alias}
              </Badge>
            ))
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          {!showDeleteConfirm ? (
            <>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setEdit(true)}
              >
                {t('assistant.card.edit')}
              </Button>

              <Button
                size="sm"
                variant="ghost"
                onClick={handleDeleteClick}
                disabled={isDeleting}
                title={
                  assistant.deletionProtected === true
                    ? t('assistant.card.deleteBlocked')
                    : t('assistant.card.delete')
                }
              >
                {t('assistant.card.delete')}
              </Button>
            </>
          ) : (
            <>
              <Button
                size="sm"
                variant="destructive"
                onClick={handleDeleteConfirm}
                disabled={isDeleting}
                className="flex-1"
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
              >
                {t('assistant.card.cancel')}
              </Button>
            </>
          )}
        </div>
      </div>
      <AssistantEditor.Dialog open={edit} onOpenChange={setEdit} />
    </EditorProvider>
  );
}
