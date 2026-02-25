import React from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import { createId } from '@paralleldrive/cuid2';
import { Button, Input, Label } from '@/components/ui';
import type { MCPServerEntity } from '@/models/chat';
import type { KeyValuePair, MCPServerMetadata } from '../hooks/useMCPServerForm';

interface EnvVarsFormProps {
  server: MCPServerEntity;
  envVars: KeyValuePair[];
  onAdd: () => void;
  onRemove: (id: string) => void;
  onUpdate: (id: string, field: 'key' | 'value', value: string) => void;
  setEnvVars: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
}

export function EnvVarsForm({
  server,
  envVars,
  onAdd,
  onRemove,
  onUpdate,
  setEnvVars,
}: EnvVarsFormProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <Label>{t('mcpServer.dialog.envVarsLabel', 'Environment Variables')}</Label>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onAdd}
          className="h-7 text-xs"
        >
          <Plus className="w-3 h-3 mr-1" />{' '}
          {t('mcpServer.dialog.addEnvVar', 'Add Variable')}
        </Button>
      </div>

      {/* Defined Variables (from Preset) */}
      {(server.metadata as MCPServerMetadata | undefined)?.variableDefinitions && (
        <div className="space-y-4 mb-4 p-4 border rounded-md bg-muted/10">
          <h4 className="text-sm font-medium mb-2">
            {t('mcpServer.dialog.requiredConfig', 'Required Configuration')}
          </h4>
          {Object.entries(
            (server.metadata as MCPServerMetadata).variableDefinitions || {},
          ).map(([key, def]) => {
            const envVar = envVars.find((v) => v.key === key);
            const val = envVar?.value || '';

            return (
              <div key={key} className="space-y-2">
                <Label htmlFor={`env-${key}`} className="flex gap-1 items-center">
                  {def.label || key}
                  {def.required && <span className="text-destructive">*</span>}
                </Label>
                <Input
                  id={`env-${key}`}
                  type={def.type === 'password' ? 'password' : 'text'}
                  value={val}
                  placeholder={def.label}
                  onChange={(e) => {
                    if (envVar) {
                      onUpdate(envVar.id, 'value', e.target.value);
                    } else {
                      // If it doesn't exist, add it
                      setEnvVars((prev) => [
                        ...prev,
                        {
                          id: createId(),
                          key,
                          value: e.target.value,
                        },
                      ]);
                    }
                  }}
                />
                {def.description && (
                  <p className="text-xs text-muted-foreground">{def.description}</p>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Custom/Other Variables */}
      {envVars.filter(
        (item) =>
          !(server.metadata as MCPServerMetadata | undefined)?.variableDefinitions?.[
            item.key
          ],
      ).length === 0 ? (
        !(server.metadata as MCPServerMetadata | undefined)?.variableDefinitions && (
          <div className="text-xs text-muted-foreground italic py-2 border rounded-md border-dashed text-center bg-muted/20">
            {t(
              'mcpServer.dialog.noCustomEnv',
              'No custom environment variables configured.',
            )}
          </div>
        )
      ) : (
        <div className="space-y-2">
          <Label className="text-xs text-muted-foreground">
            {t('mcpServer.dialog.customVarsLabel', 'Custom Variables')}
          </Label>
          {envVars
            .filter(
              (item) =>
                !(server.metadata as MCPServerMetadata | undefined)
                  ?.variableDefinitions?.[item.key],
            )
            .map((item) => (
              <div key={item.id} className="flex gap-2 items-start">
                <div className="flex-1">
                  <Input
                    placeholder={t(
                      'mcpServer.dialog.envVarKeyPlaceholder',
                      'Key (e.g. API_KEY)',
                    )}
                    value={item.key}
                    onChange={(e) => onUpdate(item.id, 'key', e.target.value)}
                    className="h-8 text-sm font-mono"
                    aria-label="Environment variable key"
                  />
                </div>
                <div className="flex-1">
                  <Input
                    placeholder={t('mcpServer.dialog.envVarValuePlaceholder', 'Value')}
                    value={item.value}
                    onChange={(e) => onUpdate(item.id, 'value', e.target.value)}
                    type="password" // Mask values for security
                    className="h-8 text-sm font-mono"
                    aria-label="Environment variable value"
                  />
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => onRemove(item.id)}
                  aria-label={
                    item.key
                      ? t('mcpServer.dialog.removeEnvVar', {
                          key: item.key,
                          defaultValue: 'Remove environment variable {{key}}',
                        })
                      : t(
                          'mcpServer.dialog.removeUnnamedEnvVar',
                          'Remove unnamed environment variable',
                        )
                  }
                  className="h-8 w-8 text-muted-foreground hover:text-destructive"
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            ))}
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        {t(
          'mcpServer.dialog.envVarDesc',
          'Environment variables passed to the process (e.g. API Keys).',
        )}
      </p>
    </div>
  );
}
