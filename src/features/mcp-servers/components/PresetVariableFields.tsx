import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input, Label } from '@/components/ui';
import { MCPServerEntity } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { KeyValuePair, MCPServerMetadata } from '../hooks/useMCPServerForm';
import { isPrefilledVariableDefinition } from '../utils/preset-utils';

interface PresetVariableFieldsProps {
  server: MCPServerEntity;
  /**
   * `user-input` — no meaningful preset prefill (main install surface).
   * `prefilled` — has a template default (Advanced Settings).
   * `all` — every definition.
   */
  visibility?: 'user-input' | 'prefilled' | 'all';
  /**
   * Stable entity used to classify prefill (usually the dialog `server` prop).
   * Avoids live typing from moving fields between main/advanced.
   */
  prefillSource?: MCPServerEntity;
  envVars: KeyValuePair[];
  setEnvVars: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
  handleUpdateEnvVar: (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => void;
  apiKey: string;
  setApiKey: (key: string) => void;
  customHeaders: KeyValuePair[];
  setCustomHeaders: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
  handleUpdateHeader: (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => void;
  urlParams: Record<string, string>;
  setUrlParams: React.Dispatch<React.SetStateAction<Record<string, string>>>;
}

export function PresetVariableFields({
  server,
  visibility = 'all',
  prefillSource,
  envVars,
  setEnvVars,
  handleUpdateEnvVar,
  apiKey,
  setApiKey,
  customHeaders,
  setCustomHeaders,
  handleUpdateHeader,
  urlParams,
  setUrlParams,
}: PresetVariableFieldsProps) {
  const { t } = useTranslation('common');
  const metadata = (prefillSource ?? server).metadata as
    | MCPServerMetadata
    | undefined;
  const definitions = (
    (prefillSource ?? server).metadata as MCPServerMetadata | undefined
  )?.variableDefinitions;

  if (!definitions || Object.keys(definitions).length === 0) {
    return null;
  }

  const classificationSource = prefillSource ?? server;
  const entries = Object.entries(definitions).filter(([key, def]) => {
    if (visibility === 'all') return true;
    const prefilled = isPrefilledVariableDefinition(
      key,
      def,
      metadata,
      classificationSource,
    );
    return visibility === 'prefilled' ? prefilled : !prefilled;
  });

  if (entries.length === 0) {
    return null;
  }

  const isHttp =
    (server.transport.type as string) === 'http' ||
    server.transport.type === 'http-sse';

  const heading =
    visibility === 'prefilled'
      ? t('mcpServer.dialog.optionalConfig', 'Optional defaults')
      : entries.some(([, def]) => def.required)
        ? t('mcpServer.dialog.requiredConfig', 'Required Configuration')
        : t('mcpServer.dialog.configuration', 'Configuration');

  return (
    <div className="space-y-4 rounded-md border bg-muted/10 p-4">
      <h4 className="text-sm font-medium">{heading}</h4>
      {entries.map(([key, def]) => {
        const target = def.target ?? 'env';
        let currentValue = '';
        if (isHttp && target === 'bearer-token') {
          currentValue = apiKey;
        } else if (isHttp && target === 'header') {
          currentValue =
            customHeaders.find((header) => header.key === key)?.value || '';
        } else if (isHttp && target === 'url-param') {
          currentValue = urlParams[key] || '';
        } else {
          currentValue = envVars.find((item) => item.key === key)?.value || '';
        }

        return (
          <div key={key} className="space-y-2">
            <Label
              htmlFor={`preset-var-${key}`}
              className="flex gap-1 items-center"
            >
              {def.label || key}
              {def.required ? (
                <span className="text-destructive" aria-hidden>
                  *
                </span>
              ) : null}
              {def.required ? (
                <span className="sr-only">
                  {t('mcpServer.dialog.required', 'required')}
                </span>
              ) : null}
            </Label>
            <Input
              id={`preset-var-${key}`}
              type={def.type === 'password' ? 'password' : 'text'}
              value={currentValue}
              placeholder={def.label || key}
              aria-required={def.required ? true : undefined}
              onChange={(event) => {
                const value = event.target.value;
                if (isHttp && target === 'bearer-token') {
                  setApiKey(value);
                  return;
                }
                if (isHttp && target === 'header') {
                  const existing = customHeaders.find(
                    (header) => header.key === key,
                  );
                  if (existing) {
                    handleUpdateHeader(existing.id, 'value', value);
                  } else {
                    setCustomHeaders((prev) => [
                      ...prev,
                      { id: createId(), key, value },
                    ]);
                  }
                  return;
                }
                if (isHttp && target === 'url-param') {
                  setUrlParams((prev) => ({ ...prev, [key]: value }));
                  return;
                }

                const existing = envVars.find((item) => item.key === key);
                if (existing) {
                  handleUpdateEnvVar(existing.id, 'value', value);
                } else {
                  setEnvVars((prev) => [
                    ...prev,
                    { id: createId(), key, value },
                  ]);
                }
              }}
            />
            {def.description ? (
              <p className="text-xs text-muted-foreground">{def.description}</p>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
