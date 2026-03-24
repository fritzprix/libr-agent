import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { useCallback, useMemo } from 'react';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { useTranslation } from 'react-i18next';
import {
  CORE_BUILTIN_SERVICE_ALIASES,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '@/lib/assistant/runtime-builtins';
import { useBuiltinTools } from './useBuiltinTools';

export default function BuiltInToolsEditor() {
  const { draft, update } = useEditor<Assistant>();
  const { services, isLoading } = useBuiltinTools();
  const { t } = useTranslation('common');

  const allowedAliases = draft.allowedBuiltInServiceAliases;

  const allServiceAliases = useMemo(
    () => services.map((service) => service.name),
    [services],
  );

  const optionalServices = useMemo(
    () =>
      services.filter((service) =>
        OPTIONAL_BUILTIN_SERVICE_ALIASES.includes(
          service.name as (typeof OPTIONAL_BUILTIN_SERVICE_ALIASES)[number],
        ),
      ),
    [services],
  );

  const optionalServiceAliases = useMemo(
    () => optionalServices.map((service) => service.name),
    [optionalServices],
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
        const currentAliases = draft.allowedBuiltInServiceAliases;
        const effectiveCurrentAliases = currentAliases ?? allServiceAliases;

        // Extract currently enabled optional services
        const currentlyEnabledOptional = optionalServiceAliases.filter((a) =>
          effectiveCurrentAliases.includes(a),
        );

        const nextOptional = enabled
          ? Array.from(new Set([...currentlyEnabledOptional, alias]))
          : currentlyEnabledOptional.filter((a) => a !== alias);

        // ALWAYS start with core services to ensure they are present and correctly named
        const nextAliases = sortAliases([
          ...CORE_BUILTIN_SERVICE_ALIASES,
          ...nextOptional,
        ]);

        draft.allowedBuiltInServiceAliases = nextAliases;
      });
    },
    [allServiceAliases, optionalServiceAliases, sortAliases, update],
  );

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Label className="text-base font-semibold">
          {t('assistant.builtin.title')}
        </Label>
        <div className="text-sm text-muted-foreground">
          {t('assistant.builtin.loading')}
        </div>
      </div>
    );
  }

  if (services.length === 0) {
    return (
      <div className="space-y-4">
        <Label className="text-base font-semibold">
          {t('assistant.builtin.title')}
        </Label>
        <div className="text-sm text-muted-foreground">
          {t('assistant.builtin.noTools')}
        </div>
      </div>
    );
  }

  if (optionalServices.length === 0) {
    return (
      <div className="space-y-4">
        <Label className="text-base font-semibold">
          {t('assistant.builtin.toolAccess')}
        </Label>
        <div className="text-sm text-muted-foreground">
          {t('assistant.builtin.noOptional')}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Label className="text-base font-semibold">
        {t('assistant.builtin.toolAccess')}
      </Label>

      <div className="text-sm text-muted-foreground">
        {t('assistant.builtin.description')}
      </div>

      <div className="space-y-3 border rounded-lg p-4">
        {optionalServices.map((service) => {
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
                  {t('assistant.builtin.toolCount', {
                    count: service.toolCount,
                  })}
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
