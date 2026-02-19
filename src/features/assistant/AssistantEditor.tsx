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
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '../../components/ui';
import LocalServicesEditor from './LocalServicesEditor';
import SkillsEditor from './SkillsEditor';
import BuiltInToolsEditor from './BuiltInToolsEditor';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Settings, Wrench, Server } from 'lucide-react';

export default function AssistantEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { t } = useTranslation('common');

  return (
    <div className="w-full">
      <Tabs defaultValue="general" className="w-full">
        <TabsList className="w-full grid grid-cols-3 mb-4">
          <TabsTrigger value="general" className="flex items-center gap-2">
            <Settings className="h-4 w-4" />
            <span>{t('assistant.tabs.general', 'General')}</span>
          </TabsTrigger>
          <TabsTrigger value="tools" className="flex items-center gap-2">
            <Wrench className="h-4 w-4" />
            <span>{t('assistant.tabs.tools', 'Tools')}</span>
          </TabsTrigger>
          <TabsTrigger value="skills" className="flex items-center gap-2">
            <Server className="h-4 w-4" />
            <span>{t('assistant.tabs.skills', 'Skills')}</span>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="general" className="space-y-4 p-4">
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
            className="min-h-52"
          />
        </TabsContent>

        <TabsContent value="tools" className="space-y-4 p-4">
          <BuiltInToolsEditor />
          <MCPServersTab />
        </TabsContent>

        <TabsContent value="skills" className="space-y-4 p-4">
          <SkillsEditor />
          <LocalServicesEditor />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function MCPServersTab() {
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
    <div className="space-y-4">
      <div>
        <Label className="text-base font-semibold">
          {t('assistant.mcp.label', 'MCP Servers')}
        </Label>
        <p className="text-sm text-muted-foreground mt-1">
          {t(
            'assistant.mcp.description',
            'Select which MCP servers this assistant can access',
          )}
        </p>
      </div>

      {activeServers.length === 0 ? (
        <div className="text-center py-8 border rounded-lg bg-muted/20">
          <Server className="h-12 w-12 mx-auto mb-3 text-muted-foreground" />
          <p className="text-sm text-muted-foreground mb-2">
            {t('assistant.mcp.noActive', 'No active MCP servers found')}
          </p>
          <Link
            to="/settings"
            className="text-sm text-primary hover:underline inline-flex items-center gap-1"
          >
            {t('assistant.mcp.addServersLink', 'Add servers in settings')} →
          </Link>
        </div>
      ) : (
        <>
          <Input
            placeholder={t(
              'assistant.mcp.searchPlaceholder',
              'Search servers...',
            )}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />

          <div className="border rounded-lg divide-y max-h-96 overflow-y-auto">
            {filteredServers.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                {t('assistant.mcp.noMatch', 'No servers match your search')}
              </p>
            ) : (
              filteredServers.map((server) => (
                <div
                  key={server.id}
                  className="flex items-start gap-3 p-3 hover:bg-accent/50 transition-colors"
                >
                  <Checkbox
                    id={`server-${server.id}`}
                    checked={draft.mcpServerIds?.includes(server.id) || false}
                    onCheckedChange={(checked) =>
                      handleServerToggle(server.id, checked as boolean)
                    }
                    className="mt-1"
                  />
                  <label
                    htmlFor={`server-${server.id}`}
                    className="flex-1 cursor-pointer"
                  >
                    <div className="font-medium">{server.name}</div>
                    {server.metadata?.description && (
                      <div className="text-sm text-muted-foreground mt-0.5">
                        {server.metadata.description}
                      </div>
                    )}
                    <div className="text-xs text-muted-foreground mt-1 font-mono">
                      {server.transport.type === 'stdio' &&
                        `${t('assistant.mcp.transport.stdio', 'stdio')}: ${server.transport.command}`}
                      {((server.transport.type as string) === 'http' ||
                        server.transport.type === 'http-sse') &&
                        `${t('assistant.mcp.transport.http', 'http')}: ${(server.transport as { url: string }).url}`}
                    </div>
                  </label>
                </div>
              ))
            )}
          </div>
        </>
      )}
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
      <DialogContent className="max-w-3xl max-h-[90vh] p-0 flex flex-col overflow-hidden">
        <DialogHeader className="flex-shrink-0 px-6 py-4 border-b">
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
        <div className="flex-shrink-0 flex justify-end gap-2 px-6 py-4 border-t bg-muted/20">
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
