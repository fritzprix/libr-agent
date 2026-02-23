import React, { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
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
import { Switch } from '@/components/ui/switch';
import { Plus, Trash2, ChevronDown, ChevronRight } from 'lucide-react';
import { createId } from '@paralleldrive/cuid2';

interface MCPServerDialogProps {
  server: MCPServerEntity;
  onSave: (server: MCPServerEntity) => Promise<void>;
  onCancel: () => void;
}

interface KeyValuePair {
  id: string;
  key: string;
  value: string;
}

interface MCPServerMetadata {
  description?: string;
  logo?: string;
  variableDefinitions?: Record<
    string,
    {
      label?: string;
      description?: string;
      required?: boolean;
      type?: string;
      target?: 'env' | 'header' | 'bearer-token' | 'url-param';
    }
  >;
  [key: string]: unknown;
}

function MCPServerDialogComponent({
  server,
  onSave,
  onCancel,
}: MCPServerDialogProps) {
  const { t } = useTranslation('common');
  const [draft, setDraft] = useState(server);
  const [isSaving, setIsSaving] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);

  // Stdio specific state
  const [argsText, setArgsText] = useState(() => {
    if (server.transport.type === 'stdio' && server.transport.args) {
      return server.transport.args.join(' ');
    }
    return '';
  });

  // Environment Variables state (Key-Value List)
  const [envVars, setEnvVars] = useState<KeyValuePair[]>(() => {
    if (server.transport.type === 'stdio' && server.transport.env) {
      return Object.entries(server.transport.env).map(([key, value]) => ({
        id: createId(),
        key,
        value: typeof value === 'string' ? value : JSON.stringify(value),
      }));
    }
    return [];
  });

  // HTTP specific state
  const [apiKey, setApiKey] = useState(() => {
    // Type guard/check for HTTP transport which supports headers
    if (
      ((server.transport.type as string) === 'http' ||
        server.transport.type === 'http-sse') &&
      'headers' in server.transport &&
      server.transport.headers
    ) {
      // Try to extract existing Bearer token
      const auth = server.transport.headers['Authorization'];
      if (auth && auth.startsWith('Bearer ')) {
        return auth.slice(7);
      }
    }
    return '';
  });

  const [customHeaders, setCustomHeaders] = useState<KeyValuePair[]>(() => {
    if (
      ((server.transport.type as string) === 'http' ||
        server.transport.type === 'http-sse') &&
      'headers' in server.transport &&
      server.transport.headers
    ) {
      return Object.entries(server.transport.headers)
        .filter(([key]) => key !== 'Authorization') // Exclude auth header managed by apiKey field
        .map(([key, value]) => ({
          id: createId(),
          key,
          value,
        }));
    }
    return [];
  });

  const [enableSSE, setEnableSSE] = useState(() => {
    if (
      ((server.transport.type as string) === 'http' ||
        server.transport.type === 'http-sse') &&
      'enableSSE' in server.transport &&
      server.transport.enableSSE !== undefined
    ) {
      return server.transport.enableSSE;
    }
    return true; // Default to true
  });

  // URL query params state (for url-param variableDefinitions)
  const [urlParams, setUrlParams] = useState<Record<string, string>>(() => {
    try {
      if (
        ((server.transport.type as string) === 'http' ||
          server.transport.type === 'http-sse') &&
        'url' in server.transport &&
        server.transport.url
      ) {
        const urlObj = new URL(server.transport.url);
        const params: Record<string, string> = {};
        urlObj.searchParams.forEach((value, key) => {
          params[key] = value;
        });
        return params;
      }
    } catch {
      // invalid URL, ignore
    }
    return {};
  });

  const [showAdvanced, setShowAdvanced] = useState(false);
  const advancedPanelId = useId();

  const isNewServer = !server.createdAt || draft.name === '';

  const isValid = () => {
    if (!draft.name.trim()) return false;

    if (draft.transport.type === 'stdio') {
      const hasCommand = !!draft.transport.command.trim();

      // Check required defined variables
      const definitions = (draft.metadata as MCPServerMetadata | undefined)
        ?.variableDefinitions;
      if (definitions) {
        const missingRequired = Object.entries(definitions).some(
          ([key, def]) => {
            if (def.required) {
              // Check if it exists in envVars AND has a value
              const v = envVars.find((item) => item.key === key);
              return !v || !v.value.trim();
            }
            return false;
          },
        );
        if (missingRequired) return false;
      }

      return hasCommand;
    } else if (
      (draft.transport.type as string) === 'http' ||
      draft.transport.type === 'http-sse'
    ) {
      if (!draft.transport.url.trim()) return false;
      const httpDefs = (server.metadata as MCPServerMetadata | undefined)
        ?.variableDefinitions;
      if (httpDefs) {
        const missingRequired = Object.entries(httpDefs).some(([key, def]) => {
          if (!def.required) return false;
          const target = def.target ?? 'env';
          if (target === 'bearer-token') return !apiKey.trim();
          if (target === 'header') {
            const h = customHeaders.find((c) => c.key === key);
            return !h || !h.value.trim();
          }
          if (target === 'url-param') return !urlParams[key]?.trim();
          return false;
        });
        if (missingRequired) return false;
      }
      return true;
    }

    return false;
  };

  const handleAddEnvVar = () => {
    setEnvVars([...envVars, { id: createId(), key: '', value: '' }]);
  };

  const handleRemoveEnvVar = (id: string) => {
    setEnvVars(envVars.filter((item) => item.id !== id));
  };

  const handleUpdateEnvVar = (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => {
    setEnvVars(
      envVars.map((item) =>
        item.id === id ? { ...item, [field]: value } : item,
      ),
    );
  };

  const handleAddHeader = () => {
    setCustomHeaders([
      ...customHeaders,
      { id: createId(), key: '', value: '' },
    ]);
  };

  const handleRemoveHeader = (id: string) => {
    setCustomHeaders(customHeaders.filter((h) => h.id !== id));
  };

  const handleUpdateHeader = (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => {
    setCustomHeaders(
      customHeaders.map((h) => (h.id === id ? { ...h, [field]: value } : h)),
    );
  };

  const handleSave = async () => {
    if (!isValid()) {
      setValidationError(
        t(
          'mcpServer.dialog.validationError',
          'Please fill in all required fields',
        ),
      );
      return;
    }

    setIsSaving(true);
    setValidationError(null);

    try {
      if (draft.transport.type === 'stdio') {
        // Construct env object from key-value pairs
        const env: Record<string, string> = {};
        envVars.forEach((item) => {
          if (item.key.trim()) {
            env[item.key.trim()] = item.value;
          }
        });

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
            env,
          },
        };
        await onSave(updatedDraft);
      } else {
        // HTTP Transport Logic
        const headers: Record<string, string> = {};

        // Add API Key as Authorization header
        if (apiKey.trim()) {
          headers['Authorization'] = `Bearer ${apiKey.trim()}`;
        }

        // Add Custom Headers
        customHeaders.forEach((h) => {
          if (h.key.trim()) {
            headers[h.key.trim()] = h.value;
          }
        });

        // Inject url-param values into the URL
        let finalUrl = (draft.transport as { url: string }).url;
        try {
          const urlObj = new URL(finalUrl);
          Object.entries(urlParams).forEach(([key, val]) => {
            if (val.trim()) urlObj.searchParams.set(key, val.trim());
          });
          finalUrl = urlObj.toString();
        } catch {
          // keep original URL if invalid
        }

        const updatedDraft: MCPServerEntity = {
          ...draft,
          transport: {
            ...draft.transport,
            type: 'http-sse',
            url: finalUrl,
            headers,
            enableSSE: enableSSE,
          } as TransportConfig,
        };

        await onSave(updatedDraft);
      }
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog open onOpenChange={onCancel}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            {isNewServer
              ? t('mcpServer.dialog.titleNew', 'Add Extension')
              : t('mcpServer.dialog.titleEdit', {
                  name: server.name,
                  defaultValue: 'Edit Extension: {{name}}',
                })}
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
              {t('mcpServer.dialog.nameLabel', 'Name')}{' '}
              <span className="text-destructive">*</span>
            </Label>
            <Input
              id="server-name"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              placeholder={t(
                'mcpServer.dialog.namePlaceholder',
                'e.g., filesystem, github, sequential-thinking',
              )}
            />
            <p className="text-xs text-muted-foreground">
              {t(
                'mcpServer.dialog.nameDesc',
                'Unique identifier for this extension',
              )}
            </p>
          </div>

          {/* Description */}
          <div className="space-y-2">
            <Label htmlFor="server-description">
              {t('mcpServer.dialog.descLabel', 'Description')}
            </Label>
            <Textarea
              id="server-description"
              value={draft.metadata?.description || ''}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  metadata: { ...draft.metadata, description: e.target.value },
                })
              }
              placeholder={t(
                'mcpServer.dialog.descPlaceholder',
                'Optional description for this extension',
              )}
              rows={2}
            />
          </div>

          {/* Logo URL */}
          <div className="space-y-2">
            <Label htmlFor="server-logo">
              {t('mcpServer.dialog.logoLabel', 'Logo URL')}
            </Label>
            <Input
              id="server-logo"
              value={draft.metadata?.logo || ''}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  metadata: {
                    ...draft.metadata,
                    logo: e.target.value || undefined,
                  },
                })
              }
              placeholder={t(
                'mcpServer.dialog.logoPlaceholder',
                'https://example.com/logo.png',
              )}
            />
            <p className="text-xs text-muted-foreground">
              {t(
                'mcpServer.dialog.logoDesc',
                'Optional icon URL displayed on the server card',
              )}
            </p>
          </div>

          {/* Transport Type */}
          <div className="space-y-2">
            <Label htmlFor="transport-type">
              {t('mcpServer.dialog.transportLabel', 'Transport Type')}{' '}
              <span className="text-destructive">*</span>
            </Label>
            <Select
              value={
                draft.transport.type === 'http-sse'
                  ? 'http'
                  : draft.transport.type
              }
              onValueChange={(type: 'stdio' | 'http') => {
                if (type === 'stdio') {
                  setDraft({
                    ...draft,
                    transport: { type: 'stdio', command: '', args: [] },
                  });
                  setArgsText('');
                  setEnvVars([]);
                } else {
                  setDraft({
                    ...draft,
                    transport: { type: 'http-sse', url: '' } as TransportConfig,
                  });
                }
              }}
            >
              <SelectTrigger id="transport-type">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="stdio">
                  {t(
                    'mcpServer.dialog.transportStdio',
                    'stdio (Local Process)',
                  )}
                </SelectItem>
                <SelectItem value="http">
                  {t('mcpServer.dialog.transportHttp', 'HTTP (Remote Server)')}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* stdio Transport Fields */}
          {draft.transport.type === 'stdio' && (
            <>
              <div className="space-y-2">
                <Label htmlFor="stdio-command">
                  {t('mcpServer.dialog.commandLabel', 'Command')}{' '}
                  <span className="text-destructive">*</span>
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
                    if (validationError) setValidationError(null);
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
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <Label>
                    {t(
                      'mcpServer.dialog.envVarsLabel',
                      'Environment Variables',
                    )}
                  </Label>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleAddEnvVar}
                    className="h-7 text-xs"
                  >
                    <Plus className="w-3 h-3 mr-1" />{' '}
                    {t('mcpServer.dialog.addEnvVar', 'Add Variable')}
                  </Button>
                </div>

                {/* Defined Variables (from Preset) */}
                {(server.metadata as MCPServerMetadata | undefined)
                  ?.variableDefinitions && (
                  <div className="space-y-4 mb-4 p-4 border rounded-md bg-muted/10">
                    <h4 className="text-sm font-medium mb-2">
                      {t(
                        'mcpServer.dialog.requiredConfig',
                        'Required Configuration',
                      )}
                    </h4>
                    {Object.entries(
                      (server.metadata as MCPServerMetadata)
                        .variableDefinitions || {},
                    ).map(([key, def]) => {
                      const envVar = envVars.find((v) => v.key === key);
                      const val = envVar?.value || '';

                      return (
                        <div key={key} className="space-y-2">
                          <Label
                            htmlFor={`env-${key}`}
                            className="flex gap-1 items-center"
                          >
                            {def.label || key}
                            {def.required && (
                              <span className="text-destructive">*</span>
                            )}
                          </Label>
                          <Input
                            id={`env-${key}`}
                            type={def.type === 'password' ? 'password' : 'text'}
                            value={val}
                            placeholder={def.label}
                            onChange={(e) => {
                              if (envVar) {
                                handleUpdateEnvVar(
                                  envVar.id,
                                  'value',
                                  e.target.value,
                                );
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
                            <p className="text-xs text-muted-foreground">
                              {def.description}
                            </p>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}

                {/* Custom/Other Variables */}
                {envVars.filter(
                  (item) =>
                    !(server.metadata as MCPServerMetadata | undefined)
                      ?.variableDefinitions?.[item.key],
                ).length === 0 ? (
                  !(server.metadata as MCPServerMetadata | undefined)
                    ?.variableDefinitions && (
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
                      {t(
                        'mcpServer.dialog.customVarsLabel',
                        'Custom Variables',
                      )}
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
                              onChange={(e) =>
                                handleUpdateEnvVar(
                                  item.id,
                                  'key',
                                  e.target.value,
                                )
                              }
                              className="h-8 text-sm font-mono"
                              aria-label="Environment variable key"
                            />
                          </div>
                          <div className="flex-1">
                            <Input
                              placeholder={t(
                                'mcpServer.dialog.envVarValuePlaceholder',
                                'Value',
                              )}
                              value={item.value}
                              onChange={(e) =>
                                handleUpdateEnvVar(
                                  item.id,
                                  'value',
                                  e.target.value,
                                )
                              }
                              type="password" // Mask values for security
                              className="h-8 text-sm font-mono"
                              aria-label="Environment variable value"
                            />
                          </div>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            onClick={() => handleRemoveEnvVar(item.id)}
                            aria-label={
                              item.key
                                ? t('mcpServer.dialog.removeEnvVar', {
                                    key: item.key,
                                    defaultValue:
                                      'Remove environment variable {{key}}',
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
            </>
          )}

          {/* HTTP Transport Fields */}
          {((draft.transport.type as string) === 'http' ||
            draft.transport.type === 'http-sse') && (
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
                    {t(
                      'mcpServer.dialog.requiredConfig',
                      'Required Configuration',
                    )}
                  </h4>
                  {Object.entries(
                    (server.metadata as MCPServerMetadata)
                      .variableDefinitions || {},
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
                          {def.required && (
                            <span className="text-destructive">*</span>
                          )}
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
                              const existing = customHeaders.find(
                                (h) => h.key === key,
                              );
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
                    {t(
                      'mcpServer.dialog.advancedSettings',
                      'Advanced Settings',
                    )}
                  </span>
                  {showAdvanced ? (
                    <ChevronDown className="w-4 h-4 text-muted-foreground" />
                  ) : (
                    <ChevronRight className="w-4 h-4 text-muted-foreground" />
                  )}
                </button>

                {showAdvanced && (
                  <div
                    id={advancedPanelId}
                    className="p-4 pt-0 space-y-4 border-t"
                  >
                    {/* Custom Headers */}
                    <div className="space-y-2 mt-4">
                      <div className="flex items-center justify-between">
                        <Label>
                          {t(
                            'mcpServer.dialog.customHeadersLabel',
                            'Custom Headers',
                          )}
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
                          {customHeaders.map((header) => (
                            <div
                              key={header.id}
                              className="flex gap-2 items-start"
                            >
                              <div className="flex-1">
                                <Input
                                  placeholder={t(
                                    'mcpServer.dialog.headerKeyPlaceholder',
                                    'Key (e.g. User-Agent)',
                                  )}
                                  value={header.key}
                                  onChange={(e) =>
                                    handleUpdateHeader(
                                      header.id,
                                      'key',
                                      e.target.value,
                                    )
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
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={isSaving}>
            {t('mcpServer.dialog.cancel', 'Cancel')}
          </Button>
          <Button onClick={handleSave} disabled={!isValid() || isSaving}>
            {isSaving
              ? t('mcpServer.dialog.saving', 'Saving...')
              : t('mcpServer.dialog.save', 'Save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export const MCPServerDialog = React.memo(
  MCPServerDialogComponent,
  (prev, next) => {
    return (
      prev.server.id === next.server.id &&
      prev.server.updatedAt?.getTime() === next.server.updatedAt?.getTime() &&
      prev.onSave === next.onSave &&
      prev.onCancel === next.onCancel
    );
  },
);
