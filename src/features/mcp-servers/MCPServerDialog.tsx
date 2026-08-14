import React, { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight } from 'lucide-react';
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
import { useMCPServerForm } from './hooks/useMCPServerForm';
import { StdioForm } from './components/StdioForm';
import { HttpForm } from './components/HttpForm';
import { PresetServerSummary } from './components/PresetServerSummary';
import { PresetVariableFields } from './components/PresetVariableFields';
import {
  isPrefilledVariableDefinition,
  isRegistrySourcedServer,
} from './utils/preset-utils';
import type { MCPServerMetadata } from './hooks/useMCPServerForm';
import { TransportConfig } from '@/lib/mcp/config/transport';

interface MCPServerDialogProps {
  server: MCPServerEntity;
  onSave: (server: MCPServerEntity) => Promise<void>;
  onCancel: () => void;
  /** Names from mcp-server.json — used to detect legacy installs without metadata.source */
  registryPresetNames?: ReadonlySet<string>;
  /** True when editing a server already stored in the DB */
  isExisting?: boolean;
}

function MCPServerDialogComponent({
  server,
  onSave,
  onCancel,
  registryPresetNames,
  isExisting = false,
}: MCPServerDialogProps) {
  const { t } = useTranslation('common');
  const advancedPanelId = useId();
  const {
    draft,
    setDraft,
    isSaving,
    validationError,
    setValidationError,
    argsText,
    setArgsText,
    envVars,
    setEnvVars,
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
    isValid,
    handleAddEnvVar,
    handleRemoveEnvVar,
    handleUpdateEnvVar,
    handleAddHeader,
    handleRemoveHeader,
    handleUpdateHeader,
    submit,
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
  } = useMCPServerForm(server);

  const isRegistryPreset = isRegistrySourcedServer(server, registryPresetNames);
  const isInstallFlow = isRegistryPreset && !isExisting;
  const variableDefinitions = server.metadata?.variableDefinitions;
  const hasUserInputVariables = Boolean(
    variableDefinitions &&
      Object.entries(variableDefinitions).some(
        ([key, def]) =>
          !isPrefilledVariableDefinition(
            key,
            def,
            server.metadata as MCPServerMetadata | undefined,
            server,
          ),
      ),
  );
  const isHttpTransport =
    (draft.transport.type as string) === 'http' ||
    draft.transport.type === 'http-sse';

  const presetVariableFieldProps = {
    server: draft,
    prefillSource: server,
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
  };

  const metadataFields = (
    <>
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
    </>
  );

  const transportSelect = (
    <div className="space-y-2">
      <Label htmlFor="transport-type">
        {t('mcpServer.dialog.transportLabel', 'Transport Type')}{' '}
        <span className="text-destructive">*</span>
      </Label>
      <Select
        value={
          draft.transport.type === 'http-sse' ? 'http' : draft.transport.type
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
            {t('mcpServer.dialog.transportStdio', 'stdio (Local Process)')}
          </SelectItem>
          <SelectItem value="http">
            {t('mcpServer.dialog.transportHttp', 'HTTP (Remote Server)')}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  );

  const stdioForm = draft.transport.type === 'stdio' && (
    <StdioForm
      draft={draft}
      setDraft={setDraft}
      argsText={argsText}
      setArgsText={setArgsText}
      setValidationError={setValidationError}
      server={server}
      envVars={envVars}
      setEnvVars={setEnvVars}
      handleAddEnvVar={handleAddEnvVar}
      handleRemoveEnvVar={handleRemoveEnvVar}
      handleUpdateEnvVar={handleUpdateEnvVar}
      hideVariableDefinitions={isRegistryPreset}
    />
  );

  const httpForm = isHttpTransport && (
    <HttpForm
      draft={draft}
      setDraft={setDraft}
      server={server}
      apiKey={apiKey}
      setApiKey={setApiKey}
      customHeaders={customHeaders}
      setCustomHeaders={setCustomHeaders}
      enableSSE={enableSSE}
      setEnableSSE={setEnableSSE}
      urlParams={urlParams}
      setUrlParams={setUrlParams}
      showAdvanced={showAdvanced}
      setShowAdvanced={setShowAdvanced}
      handleAddHeader={handleAddHeader}
      handleRemoveHeader={handleRemoveHeader}
      handleUpdateHeader={handleUpdateHeader}
      authType={authType}
      setAuthType={setAuthType}
      discoveryUrl={discoveryUrl}
      setDiscoveryUrl={setDiscoveryUrl}
      authorizationEndpoint={authorizationEndpoint}
      setAuthorizationEndpoint={setAuthorizationEndpoint}
      tokenEndpoint={tokenEndpoint}
      setTokenEndpoint={setTokenEndpoint}
      clientId={clientId}
      setClientId={setClientId}
      clientSecret={clientSecret}
      setClientSecret={setClientSecret}
      scopes={scopes}
      setScopes={setScopes}
      usePkce={usePkce}
      setUsePkce={setUsePkce}
      hideVariableDefinitions={isRegistryPreset}
      omitOuterAdvancedToggle={isRegistryPreset}
      registryAuthDetailsOnly={isRegistryPreset}
    />
  );

  const registryOAuthBasics = authType === 'oauth2.1' && (
    <div className="space-y-4 rounded-md border bg-muted/10 p-4">
      <h4 className="text-sm font-medium">
        {t('mcpServer.dialog.oauthConfigHeader', 'OAuth 2.1 Configuration')}
      </h4>
      <div className="space-y-2">
        <Label htmlFor="oauth-client-id-main">
          {t('mcpServer.dialog.clientIdLabel', 'Client Key (Client ID)')}{' '}
          <span className="text-destructive">*</span>
        </Label>
        <Input
          id="oauth-client-id-main"
          value={clientId}
          onChange={(e) => setClientId(e.target.value)}
          placeholder="e.g. slack-mcp-client-id"
          aria-required
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="oauth-client-secret-main">
          {t('mcpServer.dialog.clientSecretLabel', 'Client Secret')}{' '}
          <span className="text-muted-foreground text-xs">
            {t('mcpServer.dialog.clientSecretOptional', '(Optional)')}
          </span>
        </Label>
        <Input
          id="oauth-client-secret-main"
          type="password"
          value={clientSecret}
          onChange={(e) => setClientSecret(e.target.value)}
          placeholder="Enter client secret if using a confidential client"
        />
      </div>
      <p className="text-xs text-muted-foreground">
        {t(
          'mcpServer.dialog.oauthAdvancedHint',
          'Endpoints, scopes, and PKCE are under Advanced Settings.',
        )}
      </p>
    </div>
  );

  return (
    <Dialog open onOpenChange={(open) => !open && !isSaving && onCancel()}>
      <DialogContent
        className="max-w-2xl max-h-[90vh] overflow-y-auto"
        showCloseButton={!isSaving}
      >
        <DialogHeader>
          <DialogTitle>
            {isInstallFlow
              ? t('mcpServer.dialog.titleInstall', {
                  name: server.name || draft.name,
                  defaultValue: 'Install Extension: {{name}}',
                })
              : isExisting
                ? t('mcpServer.dialog.titleEdit', {
                    name: server.name,
                    defaultValue: 'Edit Extension: {{name}}',
                  })
                : t('mcpServer.dialog.titleNew', 'Add Extension')}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {validationError && (
            <div
              role="alert"
              className="rounded-md bg-destructive/10 p-3 text-sm text-destructive border border-destructive/20"
            >
              {validationError}
            </div>
          )}

          {isRegistryPreset ? (
            <>
              <PresetServerSummary server={draft} />

              <PresetVariableFields
                {...presetVariableFieldProps}
                visibility="user-input"
              />

              {registryOAuthBasics}

              {!hasUserInputVariables && authType !== 'oauth2.1' && (
                <p className="text-sm text-muted-foreground">
                  {t(
                    'mcpServer.dialog.noUserConfigNeeded',
                    'No additional configuration needed. Install to add this extension.',
                  )}
                </p>
              )}

              <div className="border rounded-md">
                <button
                  type="button"
                  onClick={() => setShowAdvanced(!showAdvanced)}
                  aria-expanded={showAdvanced}
                  aria-controls={advancedPanelId}
                  className="flex items-center justify-between w-full px-4 py-2 text-sm font-medium hover:bg-muted/50 transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-md"
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
                    className="space-y-4 border-t p-4 pt-4"
                  >
                    <PresetVariableFields
                      {...presetVariableFieldProps}
                      visibility="prefilled"
                    />
                    {metadataFields}
                    {transportSelect}
                    {stdioForm}
                    {httpForm}
                  </div>
                )}
              </div>
            </>
          ) : (
            <>
              {metadataFields}
              {transportSelect}
              {stdioForm}
              {httpForm}
            </>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={isSaving}>
            {t('mcpServer.dialog.cancel', 'Cancel')}
          </Button>
          <Button
            onClick={() =>
              submit(async (saved) => {
                await onSave({
                  ...saved,
                  metadata: {
                    ...saved.metadata,
                    ...(isRegistryPreset ? { source: 'registry' as const } : {}),
                  },
                });
              })
            }
            disabled={!isValid() || isSaving}
          >
            {isSaving
              ? isInstallFlow
                ? t('mcpServer.dialog.installing', 'Installing...')
                : t('mcpServer.dialog.saving', 'Saving...')
              : isInstallFlow
                ? t('mcpServer.dialog.install', 'Install')
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
      prev.onCancel === next.onCancel &&
      prev.registryPresetNames === next.registryPresetNames &&
      prev.isExisting === next.isExisting
    );
  },
);
