import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { getLogger } from '@/lib/logger';
import {
  listAvailableBuiltinServerDefinitions,
  BuiltinServerInfo,
} from '@/features/mcp/api/mcp-server-registry';

const logger = getLogger('BuiltIn');

export default function BuiltInToolsEditor() {
  const { draft, update } = useEditor<Assistant>();
  const [services, setServices] = useState<BuiltinServerInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;

    async function fetchDefinitions() {
      try {
        const defs = await listAvailableBuiltinServerDefinitions();
        if (isMounted) {
          setServices(
            defs.sort((a, b) =>
              a.metadata.displayName.localeCompare(b.metadata.displayName),
            ),
          );
          setIsLoading(false);
        }
      } catch (err) {
        logger.error('Failed to fetch builtin server definitions', err);
        if (isMounted) setIsLoading(false);
      }
    }

    fetchDefinitions();

    return () => {
      isMounted = false;
    };
  }, []);

  const allowedAliases = draft.allowedBuiltInServiceAliases;

  const allServiceAliases = useMemo(
    () => services.map((service) => service.name),
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
          draft.allowedBuiltInServiceAliases = undefined; // All enabled
          return;
        }

        draft.allowedBuiltInServiceAliases = next;
      });
    },
    [allServiceAliases, sortAliases, update],
  );

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
        Core features like web browsing and file management. Disable any tools
        you want to restrict for this assistant.
      </div>

      <div className="space-y-3 border rounded-lg p-4">
        {services.map((service) => {
          // Empty array = all enabled, otherwise check if in list
          // "name" from backend corresponds to "alias" in helper logic
          const isEnabled =
            allowedAliases === undefined ||
            allowedAliases.includes(service.name);

          return (
            <div
              key={service.name}
              className="flex items-start justify-between py-2"
            >
              <div className="flex-1">
                <div className="font-medium">
                  {service.metadata.displayName}
                </div>
                <div className="text-sm text-muted-foreground">
                  {service.metadata.description}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  {service.toolCount} tool{service.toolCount !== 1 ? 's' : ''}{' '}
                  available
                </div>
              </div>
              <Switch
                checked={isEnabled}
                onCheckedChange={(checked) =>
                  handleToggle(service.name, checked)
                }
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
