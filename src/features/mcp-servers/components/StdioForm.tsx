import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input, Label } from '@/components/ui';
import { MCPServerEntity } from '@/models/chat';
import { EnvVarsForm } from './EnvVarsForm';
import { KeyValuePair } from '../hooks/useMCPServerForm';

interface StdioFormProps {
  draft: MCPServerEntity;
  setDraft: React.Dispatch<React.SetStateAction<MCPServerEntity>>;
  argsText: string;
  setArgsText: (text: string) => void;
  setValidationError: (error: string | null) => void;
  server: MCPServerEntity;
  envVars: KeyValuePair[];
  setEnvVars: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
  handleAddEnvVar: () => void;
  handleRemoveEnvVar: (id: string) => void;
  handleUpdateEnvVar: (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => void;
}

export function StdioForm({
  draft,
  setDraft,
  argsText,
  setArgsText,
  setValidationError,
  server,
  envVars,
  setEnvVars,
  handleAddEnvVar,
  handleRemoveEnvVar,
  handleUpdateEnvVar,
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
            if (setValidationError) setValidationError(null);
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

      <EnvVarsForm
        server={server}
        envVars={envVars}
        setEnvVars={setEnvVars}
        handleAddEnvVar={handleAddEnvVar}
        handleRemoveEnvVar={handleRemoveEnvVar}
        handleUpdateEnvVar={handleUpdateEnvVar}
      />
    </>
  );
}
