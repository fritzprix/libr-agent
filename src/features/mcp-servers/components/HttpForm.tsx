import React, { useId, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
import { Button, Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import { ChevronDown, ChevronRight, Plus, Trash2 } from 'lucide-react';
import { KeyValuePair, MCPServerMetadata } from '../hooks/useMCPServerForm';
import { createId } from '@paralleldrive/cuid2';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '@/components/ui/select';

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
  authType: 'none' | 'oauth2.1';
  setAuthType: (type: 'none' | 'oauth2.1') => void;
  discoveryUrl: string;
  setDiscoveryUrl: (url: string) => void;
  authorizationEndpoint: string;
  setAuthorizationEndpoint: (url: string) => void;
  tokenEndpoint: string;
  setTokenEndpoint: (url: string) => void;
  clientId: string;
  setClientId: (id: string) => void;
  clientSecret: string;
  setClientSecret: (secret: string) => void;
  scopes: string;
  setScopes: (scopes: string) => void;
  usePkce: boolean;
  setUsePkce: (use: boolean) => void;
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
  authType,
  setAuthType,
  discoveryUrl,
  setDiscoveryUrl,
  authorizationEndpoint,
  setAuthorizationEndpoint,
  tokenEndpoint,
  setTokenEndpoint,
  clientId,
  setClientId,
  clientSecret,
  setClientSecret,
  scopes,
  setScopes,
  usePkce,
  setUsePkce,
}: HttpFormProps) {
  const { t } = useTranslation('common');
  const advancedPanelId = useId();
  const isAddingRef = useRef(false);

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

      {/* Authentication Method Selector */}
      <div className="space-y-2">
        <Label htmlFor="auth-method">
          {t('mcpServer.dialog.authMethodLabel', 'Authentication Method')}
        </Label>
        <Select
          value={authType}
          onValueChange={(val: 'none' | 'oauth2.1') => setAuthType(val)}
        >
          <SelectTrigger id="auth-method">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">
              {t('mcpServer.dialog.authMethodNone', 'None / Token (Bearer)')}
            </SelectItem>
            <SelectItem value="oauth2.1">
              {t('mcpServer.dialog.authMethodOAuth', 'OAuth 2.1')}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Render generic API Key field ONLY when authType is 'none' and no variableDefinitions are defined */}
      {authType === 'none' &&
        !(server.metadata as MCPServerMetadata | undefined)
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

      {/* Render OAuth 2.1 Configuration Card */}
      {authType === 'oauth2.1' && (
        <div className="space-y-4 p-4 border rounded-md bg-muted/10">
          <h4 className="text-sm font-semibold flex items-center gap-2">
            🔑{' '}
            {t('mcpServer.dialog.oauthConfigHeader', 'OAuth 2.1 Configuration')}
          </h4>

          {/* Client ID / Client Key */}
          <div className="space-y-2">
            <Label htmlFor="oauth-client-id">
              {t('mcpServer.dialog.clientIdLabel', 'Client Key (Client ID)')}{' '}
              <span className="text-destructive">*</span>
            </Label>
            <Input
              id="oauth-client-id"
              value={clientId}
              onChange={(e) => setClientId(e.target.value)}
              placeholder="e.g. slack-mcp-client-id"
            />
          </div>

          {/* Client Secret */}
          <div className="space-y-2">
            <Label htmlFor="oauth-client-secret">
              {t('mcpServer.dialog.clientSecretLabel', 'Client Secret')}{' '}
              <span className="text-muted-foreground text-xs">
                {t('mcpServer.dialog.clientSecretOptional', '(Optional)')}
              </span>
            </Label>
            <Input
              id="oauth-client-secret"
              type="password"
              value={clientSecret}
              onChange={(e) => setClientSecret(e.target.value)}
              placeholder="Enter client secret if using a confidential client"
            />
          </div>

          {/* Configuration Mode: Discovery URL or Manual Endpoints */}
          <div className="space-y-4 pt-2 border-t border-border/50">
            <div className="space-y-2">
              <Label htmlFor="oauth-discovery-url">
                {t('mcpServer.dialog.discoveryUrlLabel', 'Discovery URL')}
              </Label>
              <Input
                id="oauth-discovery-url"
                value={discoveryUrl}
                onChange={(e) => setDiscoveryUrl(e.target.value)}
                placeholder="https://mcp.example.com/.well-known/oauth-authorization-server"
              />
              <p className="text-xs text-muted-foreground">
                RFC 8414 OAuth Authorization Server metadata endpoint (takes
                precedence).
              </p>
            </div>

            <div className="text-xs text-center text-muted-foreground font-medium py-1">
              — {t('mcpServer.dialog.or', 'OR')} —
            </div>

            {/* Manual Endpoints */}
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="oauth-auth-endpoint">
                  {t(
                    'mcpServer.dialog.authEndpointLabel',
                    'Authorization Endpoint',
                  )}
                </Label>
                <Input
                  id="oauth-auth-endpoint"
                  value={authorizationEndpoint}
                  onChange={(e) => setAuthorizationEndpoint(e.target.value)}
                  placeholder="https://slack.com/oauth/v2_user/authorize"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="oauth-token-endpoint">
                  {t('mcpServer.dialog.tokenEndpointLabel', 'Token Endpoint')}
                </Label>
                <Input
                  id="oauth-token-endpoint"
                  value={tokenEndpoint}
                  onChange={(e) => setTokenEndpoint(e.target.value)}
                  placeholder="https://slack.com/api/oauth.v2.user.access"
                />
              </div>
            </div>
          </div>

          {/* Scopes */}
          <div className="space-y-2 pt-2 border-t border-border/50">
            <Label htmlFor="oauth-scopes">
              {t('mcpServer.dialog.scopesLabel', 'Scopes')}
            </Label>
            <Input
              id="oauth-scopes"
              value={scopes}
              onChange={(e) => setScopes(e.target.value)}
              placeholder="e.g. search:read.public, chat:write"
            />
            <p className="text-xs text-muted-foreground">
              {t(
                'mcpServer.dialog.scopesDesc',
                'Comma-separated scopes. Scope names cannot contain commas.',
              )}
            </p>
          </div>

          {/* PKCE Toggle */}
          <div className="flex items-center justify-between pt-2">
            <div className="space-y-0.5">
              <Label htmlFor="oauth-use-pkce">
                {t(
                  'mcpServer.dialog.usePkceLabel',
                  'Use PKCE (Proof Key for Code Exchange)',
                )}
              </Label>
              <p className="text-xs text-muted-foreground font-light">
                {usePkce
                  ? t(
                      'mcpServer.dialog.usePkceEnabledDesc',
                      'Recommended for public clients without a client secret.',
                    )
                  : t(
                      'mcpServer.dialog.usePkceDisabledDesc',
                      'Only disable PKCE for providers that do not support it. Confidential clients should use a client secret instead.',
                    )}
              </p>
            </div>
            <Switch
              id="oauth-use-pkce"
              checked={usePkce}
              onCheckedChange={setUsePkce}
            />
          </div>
        </div>
      )}

      {/* Advanced Settings */}
      <div className="border rounded-md">
        <button
          type="button"
          onClick={() => setShowAdvanced(!showAdvanced)}
          aria-expanded={showAdvanced}
          aria-controls={advancedPanelId}
          className="flex items-center justify-between w-full px-4 py-2 text-sm font-medium hover:bg-muted/50 transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-md"
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
                  onClick={() => {
                    isAddingRef.current = true;
                    handleAddHeader();
                  }}
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
                  {customHeaders.map((header, index, arr) => {
                    const removeLabel = header.key
                      ? t('mcpServer.dialog.removeHeader', {
                          key: header.key,
                          defaultValue: 'Remove header {{key}}',
                        })
                      : t(
                          'mcpServer.dialog.removeUnnamedHeader',
                          'Remove unnamed header',
                        );
                    return (
                      <div key={header.id} className="flex gap-2 items-start">
                        <div className="flex-1">
                          <Input
                            ref={(el) => {
                              if (
                                index === arr.length - 1 &&
                                isAddingRef.current &&
                                el
                              ) {
                                el.focus();
                                isAddingRef.current = false;
                              }
                            }}
                            id={`header-key-${header.id}`}
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
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon"
                              onClick={() => handleRemoveHeader(header.id)}
                              aria-label={removeLabel}
                              className="h-8 w-8 text-muted-foreground hover:text-destructive"
                            >
                              <Trash2 className="w-4 h-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{removeLabel}</TooltipContent>
                        </Tooltip>
                      </div>
                    );
                  })}
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
