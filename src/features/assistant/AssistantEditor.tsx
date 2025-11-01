import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { DialogProps } from '@radix-ui/react-dialog';
import { useEffect, useState } from 'react';
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
import { Link } from 'react-router';

export default function AssistantEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { activeServers } = useMCPServerRegistry();
  const [searchQuery, setSearchQuery] = useState('');

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
            label="Assistant Name *"
            value={draft?.name || ''}
            onChange={(e) =>
              update((draft) => {
                draft.name = e.target.value;
              })
            }
            placeholder="Enter assistant name..."
          />

          <TextareaWithLabel
            label="System Prompt *"
            value={draft?.systemPrompt || ''}
            onChange={(e) =>
              update((draft) => {
                draft.systemPrompt = e.target.value;
              })
            }
            placeholder="Describe the AI's role and behavior..."
            className="h-32"
          />

          <BuiltInToolsEditor />

          <LocalServicesEditor />

          {/* MCP Server Selection UI */}
          <div className="space-y-2">
            <Label>MCP Servers</Label>

            {activeServers.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No active MCP servers.{' '}
                <Link to="/settings" className="underline">
                  Add servers in Settings
                </Link>
              </p>
            ) : (
              <>
                <Input
                  placeholder="Search servers..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="mb-2"
                />

                <div className="max-h-64 overflow-y-auto border rounded-md p-2 space-y-2">
                  {filteredServers.length === 0 ? (
                    <p className="text-sm text-muted-foreground text-center py-4">
                      No servers match your search
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
                              `stdio: ${server.transport.command}`}
                            {server.transport.type === 'http' &&
                              `http: ${server.transport.url}`}
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
  const [open, setOpen] = useState(props.open ?? false);

  // Keep dialog open state in sync with props
  useEffect(() => {
    if (typeof props.open === 'boolean') setOpen(props.open);
  }, [props.open]);

  const handleSave = () => {
    commit();
    setOpen(false);
    if (props.onOpenChange) props.onOpenChange(false);
  };
  const handleCancel = () => {
    setOpen(false);
    if (props.onOpenChange) props.onOpenChange(false);
  };

  return (
    <Dialog {...props} open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-2xl max-h-[85vh] p-0 flex flex-col overflow-hidden">
        <DialogHeader className="flex-shrink-0 p-4 border-b">
          <DialogTitle>
            {draft.id ? '어시스턴트 편집' : '새 어시스턴트 만들기'}
          </DialogTitle>
        </DialogHeader>
        <DialogDescription className="flex-1 overflow-y-auto min-h-0">
          <AssistantEditor />
        </DialogDescription>
        <div className="flex-shrink-0 flex justify-end gap-2 p-4 border-t">
          <Button variant="outline" onClick={handleCancel}>
            취소
          </Button>
          <Button variant="default" onClick={handleSave}>
            저장
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

AssistantEditor.Dialog = AssistantDialog;
