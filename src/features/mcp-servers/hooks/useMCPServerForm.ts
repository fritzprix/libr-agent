import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
import { createId } from '@paralleldrive/cuid2';

/**
 * Builtin service group names reserved for internal tools.
 * External MCP servers must not use these names to avoid tool name collisions.
 * Keep in sync with BuiltinServiceId::from_alias() in src-tauri/src/mcp/builtin/service_id.rs
 */
const RESERVED_BUILTIN_NAMES = new Set([
  'planning',
  'workspace',
  'knowledge',
  'agent',
  'skills',
  'playbook',
  'attachments',
  'ui',
  'browser',
  'bootstrap',
  'tool',
]);

export interface KeyValuePair {
  id: string;
  key: string;
  value: string;
}

export interface MCPServerMetadata {
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

export function useMCPServerForm(server: MCPServerEntity) {
  const { t } = useTranslation('common');
  const [draft, setDraft] = useState(() => {
    const initDraft = { ...server };
    if (
      ((initDraft.transport.type as string) === 'http' ||
        initDraft.transport.type === 'http-sse') &&
      'url' in initDraft.transport &&
      initDraft.transport.url
    ) {
      try {
        const urlObj = new URL(initDraft.transport.url);
        const varDefs = (initDraft.metadata as MCPServerMetadata | undefined)
          ?.variableDefinitions;
        let changed = false;

        if (varDefs) {
          Object.entries(varDefs).forEach(([key, def]) => {
            if (def.target === 'url-param' && urlObj.searchParams.has(key)) {
              urlObj.searchParams.delete(key);
              changed = true;
            }
          });
        }

        if (changed) {
          return {
            ...initDraft,
            transport: {
              ...initDraft.transport,
              url: urlObj.toString(),
            } as TransportConfig,
          };
        }
      } catch {
        // invalid URL
      }
    }
    return initDraft;
  });
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
      // Keys managed by variableDefinitions (bearer-token uses Authorization, header uses its key)
      const managedKeys = new Set<string>(['Authorization']);
      const varDefs = (server.metadata as MCPServerMetadata | undefined)
        ?.variableDefinitions;
      if (varDefs) {
        Object.entries(varDefs).forEach(([key, def]) => {
          const target = def.target ?? 'env';
          if (target === 'header') managedKeys.add(key);
        });
      }
      return Object.entries(server.transport.headers)
        .filter(([key]) => !managedKeys.has(key))
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

        // Extract managed url-params
        const varDefs = (server.metadata as MCPServerMetadata | undefined)
          ?.variableDefinitions;
        if (varDefs) {
          Object.entries(varDefs).forEach(([key, def]) => {
            if (def.target === 'url-param') {
              const value = urlObj.searchParams.get(key);
              if (value) {
                params[key] = value;
                // We don't remove it from urlObj here because we just need to read it.
                // We'll clean up the main draft.url initialization instead.
              }
            }
          });
        }
        return params;
      }
    } catch {
      // invalid URL, ignore
    }
    return {};
  });

  const [showAdvanced, setShowAdvanced] = useState(false);

  const isNewServer = !server.createdAt || draft.name === '';

  const isReservedName = () =>
    RESERVED_BUILTIN_NAMES.has(draft.name.trim().toLowerCase());

  const isValid = () => {
    if (!draft.name.trim()) return false;
    if (isReservedName()) return false;

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

  const submit = async (onSave: (server: MCPServerEntity) => Promise<void>) => {
    if (!isValid()) {
      if (isReservedName()) {
        setValidationError(
          t(
            'mcpServer.dialog.reservedNameError',
            '"{{name}}" is a reserved builtin service name. Choose a different name.',
            { name: draft.name.trim() },
          ),
        );
      } else {
        setValidationError(
          t(
            'mcpServer.dialog.validationError',
            'Please fill in all required fields',
          ),
        );
      }
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

  return {
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
    isNewServer,
    isValid,
    handleAddEnvVar,
    handleRemoveEnvVar,
    handleUpdateEnvVar,
    handleAddHeader,
    handleRemoveHeader,
    handleUpdateHeader,
    submit,
  };
}
