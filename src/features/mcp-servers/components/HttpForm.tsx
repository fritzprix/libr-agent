import React, { useId, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
import { Button, Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import { ChevronDown, ChevronRight, Plus, Trash2 } from 'lucide-react';
import { KeyValuePair, MCPServerMetadata } from '../hooks/useMCPServerForm';
import { createId } from '@paralleldrive/cuid2';

interface HttpFormProps {
  draft: MCPServerEntity;
  setDraft: React.Dispatch<React.SetStateAction<MCPServerEntity>>;
  server: MCPServerEntity;
  apiKey: string;
  setApiKey: (key: string) => void;
  customHeaders: KeyValuePair[];
  setCustomHeaders: React.Dispatch<React.SetStateAction<KeyValuePair[]>>;
  enableSSE: boolean;
  setEnableSSE: (enable: boolean) => void;
  urlParams: Record<string, string>;
  setUrlParams: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  showAdvanced: boolean;
  setShowAdvanced: (show: boolean) => void;
  handleAddHeader: () => void;
  handleRemoveHeader: (id: string) => void;
  handleUpdateHeader: (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => void;
}

export function HttpForm({
  draft,
  setDraft,
  server,
  apiKey,
  setApiKey,
  customHeaders,
  setCustomHeaders,
  enableSSE,
  setEnableSSE,
  urlParams,
  setUrlParams,
  showAdvanced,
  setShowAdvanced,
  handleAddHeader,
  handleRemoveHeader,
  handleUpdateHeader,
}: HttpFormProps) {
  const { t } = useTranslation('common');
  const advancedPanelId = useId();
  const prevLengthRef = useRef(customHeaders.length);
  const customHeadersRef = useRef(customHeaders);
  customHeadersRef.current = customHeaders;
  const lastNewInputRef = useRef<HTMLInputElement | null>(null);

  // Auto-focus the key input of a newly added custom header.
  // Depends only on length so edits to existing items don't trigger this.
  useEffect(() => {
    if (customHeadersRef.current.length > prevLengthRef.current) {
      lastNewInputRef.current?.focus();
    }
    prevLengthRef.current = customHeadersRef.current.length;
  }, [customHeaders.length]);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="http-url">
          {t('mcpServer.dialog.urlLabel', 'URL')}{' '}
          <span className="text-destructive">*</span>
        </Label>
        <Input
          id="http-url"
          value={
            (draft.transport.type as string) === 'http' ||
            draft.transport.type === 'http-sse'
              ? (draft.transport as { url: string }).url
              : ''
          }
          onChange={(e) => {
            setDraft({
              ...draft,
              transport: {
                ...draft.transport,
                type: 'http-sse',
                url: e.target.value,
              } as TransportConfig,
            });
          }}
          placeholder={t(
            'mcpServer.dialog.urlPlaceholder',
            'https://api.example.com/mcp',
          )}
        />
        <p className="text-xs text-muted-foreground">
          {t(
            'mcpServer.dialog.urlDesc',
            'Full URL to the remote extension endpoint',
          )}
        </p>
      </div>

      {/* Required Configuration for HTTP (from variableDefinitions) */}
      {(server.metadata as MCPServerMetadata | undefined)
        ?.variableDefinitions && (
        <div className="space-y-4 p-4 border rounded-md bg-muted/10">
          <h4 className="text-sm font-medium">
            {t('mcpServer.dialog.requiredConfig', 'Required Configuration')}
          </h4>
          {Object.entries(
            (server.metadata as MCPServerMetadata).variableDefinitions || {},
          ).map(([key, def]) => {
            const target = def.target ?? 'env';
            let currentValue = '';
            if (target === 'bearer-token') {
              currentValue = apiKey;
            } else if (target === 'header') {
              currentValue =
                customHeaders.find((h) => h.key === key)?.value || '';
            } else if (target === 'url-param') {
              currentValue = urlParams[key] || '';
            }
            return (
              <div key={key} className="space-y-2">
                <Label
                  htmlFor={`http-var-${key}`}
                  className="flex gap-1 items-center"
                >
                  {def.label || key}
                  {def.required && <span className="text-destructive">*</span>}
                </Label>
                <Input
                  id={`http-var-${key}`}
                  type={def.type === 'password' ? 'password' : 'text'}
                  value={currentValue}
                  placeholder={def.label}
                  onChange={(e) => {
                    if (target === 'bearer-token') {
                      setApiKey(e.target.value);
                    } else if (target === 'header') {
                      const existing = customHeaders.find((h) => h.key === key);
                      if (existing) {
                        handleUpdateHeader(
                          existing.id,
                          'value',
                          e.target.value,
                        );
                      } else {
                        setCustomHeaders((prev) => [
                          ...prev,
                          {
                            id: createId(),
                            key,
                            value: e.target.value,
                          },
                        ]);
                      }
                    } else if (target === 'url-param') {
                      setUrlParams((prev) => ({
                        ...prev,
                        [key]: e.target.value,
                      }));
                    }
                  }}
                />
                {def.description && (
                  <p className="text-xs text-muted-foreground">
                    {def.description}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Only show the generic API Key field if no variableDefinitions are defined for this server.
                  When variableDefinitions exist, all credentials are handled through that section above. */}
      {!(server.metadata as MCPServerMetadata | undefined)
        ?.variableDefinitions && (
        <div className="space-y-2">
          <Label htmlFor="http-api-key">
            {t('mcpServer.dialog.apiKeyLabel', 'API Key / Token')}{' '}
            <span className="text-muted-foreground text-xs">
              {t('mcpServer.dialog.apiKeyOptional', '(Optional)')}
            </span>
          </Label>
          <Input
            id="http-api-key"
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={t(
              'mcpServer.dialog.apiKeyPlaceholder',
              'Secret Token',
            )}
          />
          <p className="text-xs text-muted-foreground">
            {t(
              'mcpServer.dialog.apiKeyDesc',
              "Automatically adds 'Authorization: Bearer <token>' header.",
            )}
          </p>
        </div>
      )}

      {/* Advanced Settings */}
      <div className="border rounded-md">
        <button
          type="button"
          onClick={() => setShowAdvanced(!showAdvanced)}
          aria-expanded={showAdvanced}
          aria-controls={advancedPanelId}
          className="flex items-center justify-between w-full px-4 py-2 text-sm font-medium hover:bg-muted/50 transition-colors focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none rounded-md"
        >
          <span>
            {t('mcpServer.dialog.advancedSettings', 'Advanced Settings')}
          </span>
          {showAdvanced ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground" />
          )}
        </button>

        {showAdvanced && (
          <div id={advancedPanelId} className="p-4 pt-0 space-y-4 border-t">
            {/* Custom Headers */}
            <div className="space-y-2 mt-4">
              <div className="flex items-center justify-between">
                <Label>
                  {t('mcpServer.dialog.customHeadersLabel', 'Custom Headers')}
                </Label>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleAddHeader}
                  className="h-7 text-xs"
                >
                  <Plus className="w-3 h-3 mr-1" />{' '}
                  {t('mcpServer.dialog.addHeader', 'Add Header')}
                </Button>
              </div>

              {customHeaders.length === 0 ? (
                <p className="text-xs text-muted-foreground italic py-1">
                  {t(
                    'mcpServer.dialog.noCustomHeaders',
                    'No custom headers configured.',
                  )}
                </p>
              ) : (
                <div className="space-y-2">
                  {customHeaders.map((header, index, arr) => (
                    <div key={header.id} className="flex gap-2 items-start">
                      <div className="flex-1">
                        <Input
                          ref={
                            index === arr.length - 1
                              ? lastNewInputRef
                              : undefined
                          }
                          id={`header-key-${header.id}`}
                          placeholder={t(
                            'mcpServer.dialog.headerKeyPlaceholder',
                            'Key (e.g. User-Agent)',
                          )}
                          value={header.key}
                          onChange={(e) =>
                            handleUpdateHeader(header.id, 'key', e.target.value)
                          }
                          className="h-8 text-sm"
                          aria-label="Custom header key"
                        />
                      </div>
                      <div className="flex-1">
                        <Input
                          placeholder={t(
                            'mcpServer.dialog.headerValuePlaceholder',
                            'Value',
                          )}
                          value={header.value}
                          onChange={(e) =>
                            handleUpdateHeader(
                              header.id,
                              'value',
                              e.target.value,
                            )
                          }
                          className="h-8 text-sm"
                          aria-label="Custom header value"
                        />
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => handleRemoveHeader(header.id)}
                        aria-label={
                          header.key
                            ? t('mcpServer.dialog.removeHeader', {
                                key: header.key,
                                defaultValue: 'Remove header {{key}}',
                              })
                            : t(
                                'mcpServer.dialog.removeUnnamedHeader',
                                'Remove unnamed header',
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
            </div>

            {/* SSE Toggle */}
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="enable-sse">
                  {t(
                    'mcpServer.dialog.sseLabel',
                    'Enable Server-Sent Events (SSE)',
                  )}
                </Label>
                <p className="text-xs text-muted-foreground">
                  {t(
                    'mcpServer.dialog.sseDesc',
                    'Keep enabled for streaming responses. Disable for stateless HTTP.',
                  )}
                </p>
              </div>
              <Switch
                id="enable-sse"
                checked={enableSSE}
                onCheckedChange={setEnableSSE}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
