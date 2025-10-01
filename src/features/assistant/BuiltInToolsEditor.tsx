import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { useBuiltInTool } from '@/features/tools';
import { useCallback, useMemo } from 'react';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { extractBuiltInServiceAlias } from '@/lib/utils';

interface BuiltInServiceInfo {
  alias: string;
  displayName: string;
  description: string;
  toolCount: number;
}

export default function BuiltInToolsEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { availableTools, status, getServiceMetadata } = useBuiltInTool();

  // Group tools by service alias
  const services = useMemo((): BuiltInServiceInfo[] => {
    const serviceMap = new Map<string, BuiltInServiceInfo>();

    availableTools.forEach((tool) => {
      const alias = extractBuiltInServiceAlias(tool.name);
      if (!alias) return;

      if (!serviceMap.has(alias)) {
        // Get metadata from context
        const metadata = getServiceMetadata(alias);

        serviceMap.set(alias, {
          alias,
          displayName: metadata?.displayName || alias,
          description: metadata?.description || 'No description available',
          toolCount: 0,
        });
      }
      const info = serviceMap.get(alias)!;
      info.toolCount++;
    });

    return Array.from(serviceMap.values()).sort((a, b) =>
      a.displayName.localeCompare(b.displayName),
    );
  }, [availableTools, getServiceMetadata]);

  const allowedAliases = draft.allowedBuiltInServiceAliases;

  const allServiceAliases = useMemo(
    () => services.map((service) => service.alias),
    [services],
  );

  const sortAliases = useCallback(
    (aliases: string[]): string[] => {
      const orderMap = new Map(
        allServiceAliases.map((serviceAlias, index) => [serviceAlias, index]),
      );
      return Array.from(new Set(aliases))
        .filter((alias) => orderMap.has(alias))
        .sort(
          (a, b) =>
            (orderMap.get(a) ?? Number.MAX_SAFE_INTEGER) -
            (orderMap.get(b) ?? Number.MAX_SAFE_INTEGER),
        );
    },
    [allServiceAliases],
  );

  const handleToggle = useCallback(
    (alias: string, enabled: boolean) => {
      update((draft) => {
        const current = draft.allowedBuiltInServiceAliases;

        if (!enabled) {
          const next = current
            ? current.filter((a) => a !== alias)
            : allServiceAliases.filter((a) => a !== alias);
          draft.allowedBuiltInServiceAliases = sortAliases(next);
          return;
        }

        if (current === undefined) {
          // Already enabled when restrictions are undefined
          return;
        }

        if (current.includes(alias)) {
          return;
        }

        const next = sortAliases([...current, alias]);

        if (next.length === allServiceAliases.length) {
          draft.allowedBuiltInServiceAliases = undefined;
          return;
        }

        draft.allowedBuiltInServiceAliases = next;
      });
    },
    [allServiceAliases, sortAliases, update],
  );

  // Check if any service is still loading
  const isLoading = Object.values(status).some((s) => s === 'loading');

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Label className="text-base font-semibold">Built-in Tools</Label>
        <div className="text-sm text-muted-foreground">Loading tools...</div>
      </div>
    );
  }

  if (services.length === 0) {
    return (
      <div className="space-y-4">
        <Label className="text-base font-semibold">Built-in Tools</Label>
        <div className="text-sm text-muted-foreground">
          No built-in tools available.
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Label className="text-base font-semibold">Built-in Tools</Label>

      <div className="text-sm text-muted-foreground">
  Assistants can use all built-in tools by default. Disable any services
  you want to restrict for this assistant.
      </div>

      <div className="space-y-3 border rounded-lg p-4">
        {services.map((service) => {
          // Empty array = all enabled, otherwise check if in list
          const isEnabled =
            allowedAliases === undefined ||
            allowedAliases.includes(service.alias);

          return (
            <div
              key={service.alias}
              className="flex items-start justify-between py-2"
            >
              <div className="flex-1">
                <div className="font-medium">{service.displayName}</div>
                <div className="text-sm text-muted-foreground">
                  {service.description}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  {service.toolCount} tool{service.toolCount !== 1 ? 's' : ''}{' '}
                  available
                </div>
              </div>
              <Switch
                checked={isEnabled}
                onCheckedChange={(checked) =>
                  handleToggle(service.alias, checked)
                }
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
