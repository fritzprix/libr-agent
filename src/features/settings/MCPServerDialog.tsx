import { useState } from 'react';
import { MCPServerEntity } from '@/models/chat';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  Button,
  Input,
  Textarea,
  Label,
} from '@/components/ui';
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '@/components/ui/select';

interface MCPServerDialogProps {
  server: MCPServerEntity;
  onSave: (server: MCPServerEntity) => Promise<void>;
  onCancel: () => void;
}

export function MCPServerDialog({
  server,
  onSave,
  onCancel,
}: MCPServerDialogProps) {
  const [draft, setDraft] = useState(server);
  const [isSaving, setIsSaving] = useState(false);
  const [argsText, setArgsText] = useState(() => {
    if (server.transport.type === 'stdio' && server.transport.args) {
      return server.transport.args.join(' ');
    }
    return '';
  });
  const [envJson, setEnvJson] = useState(() => {
    if (server.transport.type === 'stdio' && server.transport.env) {
      return JSON.stringify(server.transport.env, null, 2);
    }
    return '{}';
  });
  const [validationError, setValidationError] = useState<string | null>(null);

  const isNewServer = !server.createdAt || draft.name === '';

  const isValid = () => {
    if (!draft.name.trim()) return false;

    if (draft.transport.type === 'stdio') {
      return !!draft.transport.command.trim();
    } else if (draft.transport.type === 'http') {
      return !!draft.transport.url.trim();
    }

    return false;
  };

  const validateEnvJson = (): {
    valid: boolean;
    error?: string;
    env?: Record<string, string>;
  } => {
    try {
      const parsed = JSON.parse(envJson);

      // Check if it's an object (not array, null, etc.)
      if (
        typeof parsed !== 'object' ||
        Array.isArray(parsed) ||
        parsed === null
      ) {
        return {
          valid: false,
          error:
            'Environment variables must be a JSON object, e.g., {"KEY": "value"}',
        };
      }

      // Check if all values are strings
      const invalidEntries = Object.entries(parsed).filter(
        ([, value]) => typeof value !== 'string',
      );

      if (invalidEntries.length > 0) {
        const keys = invalidEntries.map(([key]) => key).join(', ');
        return {
          valid: false,
          error: `All values must be strings. Invalid keys: ${keys}`,
        };
      }

      return {
        valid: true,
        env: parsed as Record<string, string>,
      };
    } catch (err) {
      return {
        valid: false,
        error: `Invalid JSON syntax: ${err instanceof Error ? err.message : 'Unknown error'}`,
      };
    }
  };

  const handleSave = async () => {
    if (!isValid()) {
      setValidationError('Please fill in all required fields');
      return;
    }

    // Validate environment variables for stdio transport
    if (draft.transport.type === 'stdio') {
      const envValidation = validateEnvJson();
      if (!envValidation.valid) {
        setValidationError(
          envValidation.error || 'Invalid environment variables',
        );
        return;
      }

      // Parse arguments from text input
      const args = argsText.trim()
        ? argsText.trim().split(/\s+/).filter(Boolean)
        : [];

      // Update draft with validated env and parsed args before saving
      const updatedDraft: MCPServerEntity = {
        ...draft,
        transport: {
          ...draft.transport,
          args,
          env: envValidation.env,
        },
      };

      setIsSaving(true);
      try {
        setValidationError(null);
        await onSave(updatedDraft);
      } finally {
        setIsSaving(false);
      }
    } else {
      setIsSaving(true);
      try {
        setValidationError(null);
        await onSave(draft);
      } finally {
        setIsSaving(false);
      }
    }
  };

  return (
    <Dialog open onOpenChange={onCancel}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            {isNewServer ? 'Add MCP Server' : `Edit MCP Server: ${server.name}`}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* Validation Error Message */}
          {validationError && (
            <div className="rounded-md bg-destructive/10 p-3 text-sm text-destructive border border-destructive/20">
              {validationError}
            </div>
          )}

          {/* Server Name */}
          <div className="space-y-2">
            <Label htmlFor="server-name">
              Server Name <span className="text-destructive">*</span>
            </Label>
            <Input
              id="server-name"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              placeholder="e.g., filesystem, github, sequential-thinking"
            />
            <p className="text-xs text-muted-foreground">
              Unique identifier for this MCP server
            </p>
          </div>

          {/* Description */}
          <div className="space-y-2">
            <Label htmlFor="server-description">Description</Label>
            <Textarea
              id="server-description"
              value={draft.metadata?.description || ''}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  metadata: { ...draft.metadata, description: e.target.value },
                })
              }
              placeholder="Optional description for this server"
              rows={2}
            />
          </div>

          {/* Transport Type */}
          <div className="space-y-2">
            <Label htmlFor="transport-type">
              Transport Type <span className="text-destructive">*</span>
            </Label>
            <Select
              value={draft.transport.type}
              onValueChange={(type: 'stdio' | 'http') => {
                if (type === 'stdio') {
                  setDraft({
                    ...draft,
                    transport: { type: 'stdio', command: '', args: [] },
                  });
                  setArgsText('');
                  setEnvJson('{}');
                } else {
                  setDraft({
                    ...draft,
                    transport: { type: 'http', url: '' },
                  });
                }
              }}
            >
              <SelectTrigger id="transport-type">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="stdio">stdio (Local Process)</SelectItem>
                <SelectItem value="http">HTTP (Remote Server)</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* stdio Transport Fields */}
          {draft.transport.type === 'stdio' && (
            <>
              <div className="space-y-2">
                <Label htmlFor="stdio-command">
                  Command <span className="text-destructive">*</span>
                </Label>
                <Input
                  id="stdio-command"
                  value={
                    draft.transport.type === 'stdio'
                      ? draft.transport.command
                      : ''
                  }
                  onChange={(e) => {
                    if (draft.transport.type === 'stdio') {
                      setDraft({
                        ...draft,
                        transport: {
                          type: 'stdio',
                          command: e.target.value,
                          args: draft.transport.args,
                          env: draft.transport.env,
                        },
                      });
                    }
                  }}
                  placeholder="e.g., npx, node, python"
                />
                <p className="text-xs text-muted-foreground">
                  Executable command to start the MCP server
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="stdio-args">Arguments</Label>
                <Input
                  id="stdio-args"
                  value={argsText}
                  onChange={(e) => {
                    setArgsText(e.target.value);
                    // Clear validation error when user starts editing
                    if (validationError) {
                      setValidationError(null);
                    }
                  }}
                  placeholder="e.g., -y @modelcontextprotocol/server-filesystem /tmp"
                />
                <p className="text-xs text-muted-foreground">
                  Space-separated command arguments. Multiple spaces will be
                  normalized when saving.
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="stdio-env">Environment Variables (JSON)</Label>
                <Textarea
                  id="stdio-env"
                  value={envJson}
                  onChange={(e) => {
                    setEnvJson(e.target.value);
                    // Clear validation error when user starts editing
                    if (validationError) {
                      setValidationError(null);
                    }
                  }}
                  placeholder='{"KEY": "value"}'
                  rows={3}
                  className="font-mono text-sm"
                />
                <p className="text-xs text-muted-foreground">
                  Optional environment variables as JSON object. Will be
                  validated when saving.
                </p>
              </div>
            </>
          )}

          {/* HTTP Transport Fields */}
          {draft.transport.type === 'http' && (
            <div className="space-y-2">
              <Label htmlFor="http-url">
                URL <span className="text-destructive">*</span>
              </Label>
              <Input
                id="http-url"
                value={
                  draft.transport.type === 'http' ? draft.transport.url : ''
                }
                onChange={(e) => {
                  if (draft.transport.type === 'http') {
                    setDraft({
                      ...draft,
                      transport: {
                        type: 'http',
                        url: e.target.value,
                      },
                    });
                  }
                }}
                placeholder="https://api.example.com/mcp"
              />
              <p className="text-xs text-muted-foreground">
                Full URL to the remote MCP server endpoint
              </p>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={isSaving}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!isValid() || isSaving}>
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
