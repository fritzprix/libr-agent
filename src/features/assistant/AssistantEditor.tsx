import { useState, useMemo, useDeferredValue } from 'react';
import { useEditor } from '@/hooks/use-editor';
import { useMCPServerRegistry } from '@/features/assistant/hooks/use-mcp-server-registry';
import { Assistant } from '@/models/assistant';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Checkbox } from '@/components/ui/checkbox';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogProps,
} from '@/components/ui/dialog';
import { useTranslation } from 'react-i18next';
import { Server } from 'lucide-react';
import { Link } from 'react-router-dom';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AssistantEditor');

export function AssistantEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { t } = useTranslation('common');

  return (
    <div className="flex flex-col h-full bg-background">
      <div className="flex-1 overflow-y-auto p-6 space-y-8">
        {/* Basic Info */}
        <section className="space-y-4">
          <div className="grid gap-2">
            <Label htmlFor="name" className="text-sm font-semibold">
              {t('assistant.nameLabel')}
            </Label>
            <Input
              id="name"
              placeholder={t('assistant.namePlaceholder')}
              value={draft.name || ''}
              onChange={(e) =>
                update((d) => {
                  d.name = e.target.value;
                })
              }
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="description" className="text-sm font-semibold">
              {t('assistant.descriptionLabel')}
            </Label>
            <Input
              id="description"
              placeholder={t('assistant.descriptionPlaceholder')}
              value={draft.description || ''}
              onChange={(e) =>
                update((d) => {
                  d.description = e.target.value;
                })
              }
            />
          </div>
        </section>

        {/* System Prompt */}
        <section className="space-y-4">
          <div className="grid gap-2">
            <Label htmlFor="systemPrompt" className="text-sm font-semibold">
              {t('assistant.systemPromptLabel')}
            </Label>
            <textarea
              id="systemPrompt"
              className="flex min-h-[200px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-none font-mono"
              placeholder={t('assistant.systemPromptPlaceholder')}
              value={draft.systemPrompt || ''}
              onChange={(e) =>
                update((d) => {
                  d.systemPrompt = e.target.value;
                })
              }
            />
          </div>
        </section>

        {/* MCP Servers */}
        <section className="pt-4 border-t">
          <MCPServersTab />
        </section>
      </div>
    </div>
  );
}

function MCPServersTab() {
  const { draft, update } = useEditor<Assistant>();
  const { activeServers } = useMCPServerRegistry();
  const [searchQuery, setSearchQuery] = useState('');
  const { t } = useTranslation('common');

  const deferredSearchQuery = useDeferredValue(searchQuery);
  const isPending = searchQuery !== deferredSearchQuery;

  const filteredServers = useMemo(() => {
    const query = deferredSearchQuery.toLowerCase().trim();
    if (!query) return activeServers;

    return activeServers.filter(
      (s) =>
        s.name.toLowerCase().includes(query) ||
        s.metadata?.description?.toLowerCase().includes(query),
    );
  }, [activeServers, deferredSearchQuery]);

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
          {t('assistant.mcp.label')}
        </Label>
        <p className="text-sm text-muted-foreground mt-1">
          {t('assistant.mcp.description')}
        </p>
      </div>

      {activeServers.length === 0 ? (
        <div className="text-center py-8 border rounded-lg bg-muted/20">
          <Server className="h-12 w-12 mx-auto mb-3 text-muted-foreground" />
          <p className="text-sm text-muted-foreground mb-2">
            {t('assistant.mcp.noActive')}
          </p>
          <Link
            to="/settings"
            className="text-sm text-primary hover:underline inline-flex items-center gap-1"
          >
            {t('assistant.mcp.addServersLink')} →
          </Link>
        </div>
      ) : (
        <>
          <Input
            placeholder={t('assistant.mcp.searchPlaceholder')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />

          <div
            className={`border rounded-lg divide-y max-h-96 overflow-y-auto transition-opacity duration-200 ${isPending ? 'opacity-50' : 'opacity-100'}`}
            aria-busy={isPending}
          >
            {isPending && (
              <span className="sr-only" aria-live="polite">
                {t('assistant.mcp.filtering')}
              </span>
            )}
            {filteredServers.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                {t('assistant.mcp.noMatch')}
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
                        `${t('assistant.mcp.transport.stdio')}: ${server.transport.command}`}
                      {((server.transport.type as string) === 'http' ||
                        server.transport.type === 'http-sse') &&
                        `${t('assistant.mcp.transport.http')}: ${(server.transport as { url: string }).url}`}
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
  const { draft, commit, isLoading } = useEditor<Assistant>();
  const { t } = useTranslation('common');

  const handleSave = async () => {
    try {
      await commit();
      if (props.onOpenChange) props.onOpenChange(false);
    } catch {
      // commit handles error logging and state internally
    }
  };
  const handleCancel = () => {
    if (props.onOpenChange) props.onOpenChange(false);
  };

  const handleOpenChange = (open: boolean) => {
    // Block close gestures (Esc, outside click) while a save is in progress
    if (!open && isLoading) return;
    if (props.onOpenChange) props.onOpenChange(open);
  };

  return (
    <Dialog {...props} open={props.open} onOpenChange={handleOpenChange}>
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
          <Button variant="outline" onClick={handleCancel} disabled={isLoading}>
            {t('assistant.edit.cancel')}
          </Button>
          <Button variant="default" onClick={handleSave} disabled={isLoading}>
            {isLoading ? t('settings.saving') : t('assistant.edit.save')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

AssistantEditor.Dialog = AssistantDialog;
