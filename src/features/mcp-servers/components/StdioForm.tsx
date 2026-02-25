import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input, Label } from '@/components/ui';
import type { MCPServerEntity } from '@/models/chat';
import type { KeyValuePair } from '../hooks/useMCPServerForm';
import { EnvVarsForm } from './EnvVarsForm';

interface StdioFormProps {
  draft: MCPServerEntity;
  setDraft: React.Dispatch<React.SetStateAction<MCPServerEntity>>;
  argsText: string;
  setArgsText: (value: string) => void;
  envVars: KeyValuePair[];
  setEnvVars: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
  onAddEnvVar: () => void;
  onRemoveEnvVar: (id: string) => void;
  onUpdateEnvVar: (id: string, field: 'key' | 'value', value: string) => void;
  setValidationError: React.Dispatch<React.SetStateAction<string | null>>;
}

export function StdioForm({
  draft,
  setDraft,
  argsText,
  setArgsText,
  envVars,
  setEnvVars,
  onAddEnvVar,
  onRemoveEnvVar,
  onUpdateEnvVar,
  setValidationError,
}: StdioFormProps) {
  const { t } = useTranslation('common');

  return (
    <>
      <div className="space-y-2">
        <Label htmlFor="stdio-command">
          {t('mcpServer.dialog.commandLabel', 'Command')}{' '}
          <span className="text-destructive">*</span>
        </Label>
        <Input
          id="stdio-command"
          value={
            draft.transport.type === 'stdio' ? draft.transport.command : ''
          }
          onChange={(e) => {
            if (draft.transport.type === 'stdio') {
              setDraft((prev) => ({
                ...prev,
                transport: {
                  ...prev.transport,
                  command: e.target.value,
                } as any, // Type assertion due to discriminated union complexity
              }));
            }
          }}
          placeholder={t(
            'mcpServer.dialog.commandPlaceholder',
            'e.g., npx, node, python',
          )}
        />
        <p className="text-xs text-muted-foreground">
          {t(
            'mcpServer.dialog.commandDesc',
            'Executable command to start the extension',
          )}
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="stdio-args">
          {t('mcpServer.dialog.argsLabel', 'Arguments')}
        </Label>
        <Input
          id="stdio-args"
          value={argsText}
          onChange={(e) => {
            setArgsText(e.target.value);
            setValidationError(null);
          }}
          placeholder={t(
            'mcpServer.dialog.argsPlaceholder',
            'e.g., -y @modelcontextprotocol/server-filesystem /tmp',
          )}
        />
        <p className="text-xs text-muted-foreground">
          {t(
            'mcpServer.dialog.argsDesc',
            'Space-separated command arguments. Multiple spaces will be normalized when saving.',
          )}
        </p>
      </div>

      {/* Environment Variables List */}
      <EnvVarsForm
        server={draft}
        envVars={envVars}
        onAdd={onAddEnvVar}
        onRemove={onRemoveEnvVar}
        onUpdate={onUpdateEnvVar}
        setEnvVars={setEnvVars}
      />
    </>
  );
}
