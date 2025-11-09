import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { DialogProps } from '@radix-ui/react-dialog';
import { useState } from 'react';
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  InputWithLabel,
  TextareaWithLabel,
  Input,
  Checkbox,
  Label,
} from '../../components/ui';
import LocalServicesEditor from './LocalServicesEditor';
import BuiltInToolsEditor from './BuiltInToolsEditor';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

export default function AssistantEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { activeServers } = useMCPServerRegistry();
  const [searchQuery, setSearchQuery] = useState('');
  const { t } = useTranslation('common');

  const filteredServers = activeServers.filter(
    (s) =>
      s.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      s.metadata?.description
        ?.toLowerCase()
        .includes(searchQuery.toLowerCase()),
  );

  const handleServerToggle = (serverId: string, enabled: boolean) => {
    update((draft) => {
      if (!draft.mcpServerIds) draft.mcpServerIds = [];

      if (enabled) {
        if (!draft.mcpServerIds.includes(serverId)) {
          draft.mcpServerIds.push(serverId);
        }
      } else {
        draft.mcpServerIds = draft.mcpServerIds.filter((id) => id !== serverId);
      }
    });
  };

  return (
    <div className="w-full">
      <div className="p-4">
        <div className="space-y-4">
          <InputWithLabel
            label={t('assistant.nameLabel')}
            value={draft?.name || ''}
            onChange={(e) =>
              update((draft) => {
                draft.name = e.target.value;
              })
            }
            placeholder={t('assistant.namePlaceholder')}
          />

          <TextareaWithLabel
            label={t('assistant.systemPromptLabel')}
            value={draft?.systemPrompt || ''}
            onChange={(e) =>
              update((draft) => {
                draft.systemPrompt = e.target.value;
              })
            }
            placeholder={t('assistant.systemPromptPlaceholder')}
            className="h-32"
          />

          <BuiltInToolsEditor />

          <LocalServicesEditor />

          {/* MCP Server Selection UI */}
          <div className="space-y-2">
            <Label>{t('assistant.mcp.label', 'MCP Servers')}</Label>

            {activeServers.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                {t('assistant.mcp.noActive')}{' '}
                <Link to="/settings" className="underline">
                  {t('assistant.mcp.addServersLink')}
                </Link>
              </p>
            ) : (
              <>
                <Input
                  placeholder={t('assistant.mcp.searchPlaceholder')}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="mb-2"
                />

                <div className="max-h-64 overflow-y-auto border rounded-md p-2 space-y-2">
                  {filteredServers.length === 0 ? (
                    <p className="text-sm text-muted-foreground text-center py-4">
                      {t('assistant.mcp.noMatch')}
                    </p>
                  ) : (
                    filteredServers.map((server) => (
                      <div
                        key={server.id}
                        className="flex items-start gap-2 p-2 hover:bg-accent rounded"
                      >
                        <Checkbox
                          id={`server-${server.id}`}
                          checked={
                            draft.mcpServerIds?.includes(server.id) || false
                          }
                          onCheckedChange={(checked) =>
                            handleServerToggle(server.id, checked as boolean)
                          }
                        />
                        <label
                          htmlFor={`server-${server.id}`}
                          className="flex-1 cursor-pointer"
                        >
                          <div className="font-medium">{server.name}</div>
                          {server.metadata?.description && (
                            <div className="text-xs text-muted-foreground">
                              {server.metadata.description}
                            </div>
                          )}
                          <div className="text-xs text-muted-foreground mt-0.5">
                            {server.transport.type === 'stdio' &&
                              `${t('assistant.mcp.transport.stdio', 'stdio')}: ${server.transport.command}`}
                            {server.transport.type === 'http' &&
                              `${t('assistant.mcp.transport.http', 'http')}: ${server.transport.url}`}
                          </div>
                        </label>
                      </div>
                    ))
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function AssistantDialog(props: DialogProps) {
  const { draft, commit } = useEditor<Assistant>();
  const { t } = useTranslation('common');

  const handleSave = () => {
    commit();
    if (props.onOpenChange) props.onOpenChange(false);
  };
  const handleCancel = () => {
    if (props.onOpenChange) props.onOpenChange(false);
  };

  return (
    <Dialog {...props} open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[85vh] p-0 flex flex-col overflow-hidden">
        <DialogHeader className="flex-shrink-0 p-4 border-b">
          <DialogTitle>
            {draft.id
              ? t('assistant.edit.titleEdit')
              : t('assistant.edit.titleNew')}
          </DialogTitle>
          <DialogDescription className="sr-only">
            Configure assistant settings, system prompt, and available tools
          </DialogDescription>
        </DialogHeader>
        <div className="flex-1 overflow-y-auto min-h-0">
          <AssistantEditor />
        </div>
        <div className="flex-shrink-0 flex justify-end gap-2 p-4 border-t">
          <Button variant="outline" onClick={handleCancel}>
            {t('assistant.edit.cancel')}
          </Button>
          <Button variant="default" onClick={handleSave}>
            {t('assistant.edit.save')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

AssistantEditor.Dialog = AssistantDialog;
