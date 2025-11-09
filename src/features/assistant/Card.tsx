import { useAssistantContext } from '@/context/AssistantContext';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useCallback, useState } from 'react';
import { Assistant } from '../../models/chat';
import { Badge, Button, StatusIndicator } from '@/components/ui';
import { EditorProvider } from '@/context/EditorContext';
import AssistantEditor from './AssistantEditor';
import { useTranslation } from 'react-i18next';

interface AssistantCardProps {
  assistant: Assistant;
}

export default function AssistantCard({ assistant }: AssistantCardProps) {
  const {
    currentAssistant,
    setCurrentAssistant,
    deleteAssistant,
    saveAssistant: upsertAssistant,
  } = useAssistantContext();
  const { status, isLoading: isCheckingStatus } = useMCPServer();
  const [isDeleting, setIsDeleting] = useState(false);
  const isActive = currentAssistant?.id === assistant.id;

  const [edit, setEdit] = useState<boolean>(false);
  const { t } = useTranslation('common');

  const handleEditComplete = useCallback(
    async (assistant: Assistant) => {
      upsertAssistant(assistant);
    },
    [upsertAssistant],
  );

  const handleDelete = async () => {
    if (assistant.isDefault) {
      alert(t('assistant.card.deleteBlocked'));
      return;
    }

    try {
      setIsDeleting(true);
      if (assistant.id) {
        await deleteAssistant(assistant.id);
      }
    } finally {
      setIsDeleting(false);
    }
  };
  return (
    <EditorProvider initialValue={assistant} onFinalize={handleEditComplete}>
      <div
        className={`border rounded p-3 cursor-pointer transition-colors ${
          isActive
            ? 'border-primary bg-primary/20'
            : 'border-muted hover:border-accent'
        }`}
        onClick={() => setCurrentAssistant(assistant)}
      >
        <div className="flex justify-between items-start mb-2">
          <h3 className="text-primary font-medium">{assistant.name}</h3>
          <div className="flex gap-1 flex-wrap">
            {assistant.isDefault && (
              <Badge variant="destructive">{t('assistant.card.default')}</Badge>
            )}
            {isActive && (
              <Badge variant="default">{t('assistant.card.active')}</Badge>
            )}
          </div>
        </div>

        <p className="text-muted-foreground text-sm mb-3 line-clamp-2">
          {assistant.systemPrompt}
        </p>

        <div className="text-xs text-muted-foreground mb-2">
          {t('assistant.card.mcpCount', {
            count: assistant.mcpServerIds?.length || 0,
          })}
          {', '}
          {t('assistant.card.localServiceCount', {
            count: assistant.localServices?.length || 0,
          })}
        </div>

        {isActive &&
          assistant.mcpServerIds &&
          assistant.mcpServerIds.length > 0 && (
            <div className="flex flex-wrap gap-1 mb-2">
              {assistant.mcpServerIds.map((serverId) => (
                <div
                  key={serverId}
                  className="flex items-center gap-1 text-xs px-1 py-0.5 rounded bg-muted"
                >
                  <StatusIndicator
                    status={
                      status[serverId] === true
                        ? 'connected'
                        : status[serverId] === false
                          ? 'disconnected'
                          : 'unknown'
                    }
                    size="sm"
                  />
                  <span className="text-foreground">{serverId}</span>
                </div>
              ))}
            </div>
          )}

        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" onClick={() => setEdit(true)}>
            {t('assistant.card.edit')}
          </Button>
          <Button size="sm" variant="ghost" disabled={isCheckingStatus}>
            {isCheckingStatus && isActive
              ? t('assistant.card.checking')
              : t('assistant.card.checkStatus')}
          </Button>
          <Button
            size="sm"
            variant="destructive"
            onClick={handleDelete}
            title={
              assistant.isDefault
                ? t('assistant.card.deleteBlocked')
                : t('assistant.card.deleteConfirmTitle')
            }
          >
            {isDeleting
              ? t('assistant.card.deleting')
              : t('assistant.card.delete')}
          </Button>
        </div>
      </div>
      <AssistantEditor.Dialog open={edit} onOpenChange={setEdit} />
    </EditorProvider>
  );
}
