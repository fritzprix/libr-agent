import React from 'react';
import { useTranslation } from 'react-i18next';
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

interface MCPServerDialogProps {
  server: MCPServerEntity;
  onSave: (server: MCPServerEntity) => Promise<void>;
  onCancel: () => void;
}

function MCPServerDialogComponent({
  server,
  onSave,
  onCancel,
}: MCPServerDialogProps) {
  const { t } = useTranslation('common');

  const {
    draft,
    setDraft,
    isSaving,
    validationError,
    setValidationError,
    isValid,
    handleSave,
    // Stdio
    argsText,
    setArgsText,
    envVars,
    setEnvVars,
    handleAddEnvVar,
    handleRemoveEnvVar,
    handleUpdateEnvVar,
    resetStdio,
    // HTTP
    apiKey,
    setApiKey,
    customHeaders,
    setCustomHeaders,
    handleAddHeader,
    handleRemoveHeader,
    handleUpdateHeader,
    enableSSE,
    setEnableSSE,
    urlParams,
    setUrlParams,
    resetHttp,
  } = useMCPServerForm(server, onSave);

  const isNewServer = !server.createdAt || draft.name === '';

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
                  resetStdio();
                } else {
                  resetHttp();
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

          {/* Transport Fields */}
          {draft.transport.type === 'stdio' ? (
            <StdioForm
              draft={draft}
              setDraft={setDraft}
              argsText={argsText}
              setArgsText={setArgsText}
              envVars={envVars}
              setEnvVars={setEnvVars}
              onAddEnvVar={handleAddEnvVar}
              onRemoveEnvVar={handleRemoveEnvVar}
              onUpdateEnvVar={handleUpdateEnvVar}
              setValidationError={setValidationError}
            />
          ) : (
            <HttpForm
              draft={draft}
              setDraft={setDraft}
              apiKey={apiKey}
              setApiKey={setApiKey}
              customHeaders={customHeaders}
              setCustomHeaders={setCustomHeaders}
              onAddHeader={handleAddHeader}
              onRemoveHeader={handleRemoveHeader}
              onUpdateHeader={handleUpdateHeader}
              enableSSE={enableSSE}
              setEnableSSE={setEnableSSE}
              urlParams={urlParams}
              setUrlParams={setUrlParams}
            />
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
