import React, { useMemo, useState } from 'react';
import { Download, Search, Server } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { MCPServerPreset } from '@/lib/backend/mcp-server-config';
import { MCPServerEntity } from '@/models/chat';
import { Button, Input } from '@/components/ui';
import { buttonVariants } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import {
  PRESET_CATEGORY_META,
  PRESET_CATEGORY_ORDER,
  type PresetCategory,
  normalizePresetCategory,
} from '../utils/preset-categories';

interface RecommendedPresetsProps {
  presets: MCPServerPreset[] | undefined;
  servers: MCPServerEntity[];
  allServers: MCPServerEntity[];
  registryLoaded: boolean;
  registryError?: string;
  onSetupPreset: (preset: MCPServerPreset) => void;
  onRetryRegistryLoad: () => Promise<void>;
}

type CategorizedPreset = MCPServerPreset & {
  category: PresetCategory;
};

export const RecommendedPresets: React.FC<RecommendedPresetsProps> = ({
  presets,
  servers,
  allServers,
  registryLoaded,
  registryError,
  onSetupPreset,
  onRetryRegistryLoad,
}) => {
  const { t } = useTranslation('common');
  const [searchQuery, setSearchQuery] = useState('');
  const [activeCategory, setActiveCategory] = useState<'all' | PresetCategory>(
    'all',
  );
  const installedServerNames = useMemo(() => {
    const names = new Set<string>();

    for (const server of servers) {
      names.add(server.name);
    }

    for (const server of allServers) {
      names.add(server.name);
    }

    return names;
  }, [allServers, servers]);

  const normalizedPresets = useMemo(
    () =>
      (presets ?? []).map((preset) => ({
        ...preset,
        category: normalizePresetCategory(preset.category),
      })),
    [presets],
  );

  const categoryCounts = useMemo(() => {
    return normalizedPresets.reduce<Record<PresetCategory, number>>(
      (counts, preset) => {
        counts[preset.category] += 1;
        return counts;
      },
      {
        search: 0,
        ai: 0,
        devtools: 0,
        data: 0,
        documents: 0,
        creative: 0,
        other: 0,
      },
    );
  }, [normalizedPresets]);

  const trimmedSearchQuery = searchQuery.trim().toLowerCase();

  const filteredPresets = useMemo(() => {
    return normalizedPresets.filter((preset) => {
      const matchesCategory =
        activeCategory === 'all' || preset.category === activeCategory;
      if (!matchesCategory) {
        return false;
      }

      if (!trimmedSearchQuery) {
        return true;
      }

      const categoryMeta = PRESET_CATEGORY_META[preset.category];
      const searchText = [
        preset.name,
        preset.description ?? '',
        categoryMeta.defaultLabel,
      ]
        .join(' ')
        .toLowerCase();

      return searchText.includes(trimmedSearchQuery);
    });
  }, [activeCategory, normalizedPresets, trimmedSearchQuery]);

  const groupedFilteredPresets = useMemo(() => {
    return filteredPresets.reduce<Record<PresetCategory, CategorizedPreset[]>>(
      (groups, preset) => {
        groups[preset.category].push(preset);
        return groups;
      },
      {
        search: [],
        ai: [],
        devtools: [],
        data: [],
        documents: [],
        creative: [],
        other: [],
      },
    );
  }, [filteredPresets]);

  const shouldGroupByCategory =
    activeCategory === 'all' && trimmedSearchQuery.length === 0;

  if (normalizedPresets.length === 0) {
    return null;
  }

  const renderPresetCard = (preset: CategorizedPreset) => {
    const isInstalled = installedServerNames.has(preset.name);
    const canInstall = !isInstalled && registryLoaded;
    const canRetryRegistryLoad =
      !isInstalled && !registryLoaded && !!registryError;
    const categoryMeta = PRESET_CATEGORY_META[preset.category];
    const CategoryIcon = categoryMeta.icon;
    const transportLabel = t(
      `assistant.mcp.transport.${preset.transportType}`,
      preset.transportType,
    );
    const presetCommandPreview = preset.command
      ? `${preset.command} ${preset.args?.[0] ?? ''}`.trim()
      : (preset.url ?? t('assistant.mcp.transport.sse', 'sse'));

    return (
      <div
        key={preset.name}
        className={cn(
          'group relative flex flex-col justify-between rounded-[1.5rem] border bg-background/50 backdrop-blur-sm p-5 transition-all duration-300 focus-visible:ring-2 focus-visible:ring-primary/20 outline-none',
          isInstalled
            ? 'opacity-60 cursor-default bg-muted/20 border-border/50'
            : canInstall
              ? 'hover:shadow-2xl hover:bg-background hover:-translate-y-1 cursor-pointer border-border/50 hover:border-primary/40'
              : 'opacity-60 cursor-wait border-border/50',
        )}
        role={canInstall ? 'button' : undefined}
        tabIndex={canInstall ? 0 : -1}
        aria-disabled={!canInstall}
        aria-label={
          canInstall
            ? t('mcpServer.installExtension', {
                name: preset.name,
                defaultValue: 'Install {{name}} extension',
              })
            : undefined
        }
        onClick={() => canInstall && onSetupPreset(preset)}
        onKeyDown={(event) => {
          if (!canInstall) return;
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            onSetupPreset(preset);
          }
        }}
      >
        <div className="space-y-1.5">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <div className="w-6 h-6 rounded overflow-hidden flex-shrink-0 border border-border/50">
                {preset.logo ? (
                  <img
                    src={preset.logo}
                    alt={preset.name}
                    className="w-full h-full object-contain"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display =
                        'none';
                      (
                        e.currentTarget.nextElementSibling as HTMLElement | null
                      )?.classList.remove('hidden');
                    }}
                  />
                ) : null}
                <div
                  className={`w-full h-full bg-muted flex items-center justify-center ${preset.logo ? 'hidden' : ''}`}
                >
                  <Server className="w-3 h-3 text-muted-foreground" />
                </div>
              </div>
              <h4 className="font-semibold tracking-tight truncate">
                {preset.name}
              </h4>
            </div>
            {isInstalled ? (
              <span className="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded font-medium flex items-center gap-1">
                {t('mcpServer.installed', 'Installed')}
              </span>
            ) : canInstall ? (
              <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground uppercase">
                {transportLabel}
              </span>
            ) : canRetryRegistryLoad ? (
              <span className="text-[10px] bg-destructive/10 px-1.5 py-0.5 rounded text-destructive uppercase">
                {t('common.retry', 'Retry')}
              </span>
            ) : (
              <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground uppercase">
                {t('common.loading', 'Loading')}
              </span>
            )}
          </div>
          <div className="flex items-center gap-1.5 text-[10px] uppercase tracking-wide text-muted-foreground">
            <CategoryIcon className="w-3 h-3" />
            <span>{t(categoryMeta.labelKey, categoryMeta.defaultLabel)}</span>
          </div>
          <p className="text-xs text-muted-foreground line-clamp-2">
            {preset.description ||
              t('mcpServer.noDescription', 'No description available')}
          </p>
        </div>
        {canInstall ? (
          <div className="mt-3 pt-3 border-t border-border/50 flex items-center justify-between opacity-60 group-hover:opacity-100 group-focus-visible:opacity-100 group-focus-within:opacity-100 transition-opacity gap-3">
            <code className="text-[10px] bg-muted px-1 py-0.5 rounded font-mono text-muted-foreground truncate">
              {presetCommandPreview}
            </code>
            <Tooltip>
              <TooltipTrigger asChild>
                <div
                  className={cn(
                    buttonVariants({ variant: 'ghost', size: 'icon' }),
                    'h-6 w-6 rounded-full group-hover:bg-primary/10 group-hover:text-primary transition-colors',
                  )}
                  aria-hidden="true"
                >
                  <Download className="w-3.5 h-3.5" />
                </div>
              </TooltipTrigger>
              <TooltipContent>
                {t('mcpServer.installExtension', {
                  name: preset.name,
                  defaultValue: 'Install {{name}} extension',
                })}
              </TooltipContent>
            </Tooltip>
          </div>
        ) : canRetryRegistryLoad ? (
          <div className="mt-3 pt-3 border-t border-border/50 flex items-center justify-between gap-3">
            <p className="text-[11px] text-muted-foreground">
              {t(
                'mcpServer.registryLoadFailed',
                'Could not verify installed extensions yet.',
              )}
            </p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={(event) => {
                event.stopPropagation();
                void onRetryRegistryLoad();
              }}
            >
              {t('common.retry', 'Retry')}
            </Button>
          </div>
        ) : null}
      </div>
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Server className="w-4 h-4 text-primary" />
        <h3 className="text-sm font-bold uppercase tracking-widest text-muted-foreground font-sans">
          {t('mcpServer.recommended', 'Recommended Extensions')}
        </h3>
        <div className="h-px bg-border/50 flex-1 ml-2" />
      </div>
      <div className="rounded-[1.5rem] border border-border/60 bg-muted/10 p-4 space-y-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 w-4 h-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder={t(
              'mcpServer.searchRecommended',
              'Search recommended extensions...',
            )}
            className="pl-9"
          />
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            variant={activeCategory === 'all' ? 'default' : 'outline'}
            onClick={() => setActiveCategory('all')}
          >
            {t('mcpServer.categories.all', 'All')}
            <span className="ml-2 rounded-full bg-background/20 px-1.5 py-0.5 text-[10px]">
              {normalizedPresets.length}
            </span>
          </Button>

          {PRESET_CATEGORY_ORDER.filter(
            (category) => categoryCounts[category] > 0,
          ).map((category) => {
            const categoryMeta = PRESET_CATEGORY_META[category];
            const CategoryIcon = categoryMeta.icon;

            return (
              <Button
                key={category}
                type="button"
                size="sm"
                variant={activeCategory === category ? 'default' : 'outline'}
                onClick={() => setActiveCategory(category)}
              >
                <CategoryIcon className="w-3.5 h-3.5 mr-1.5" />
                {t(categoryMeta.labelKey, categoryMeta.defaultLabel)}
                <span className="ml-2 rounded-full bg-background/20 px-1.5 py-0.5 text-[10px]">
                  {categoryCounts[category]}
                </span>
              </Button>
            );
          })}

          {(searchQuery || activeCategory !== 'all') && (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => {
                setSearchQuery('');
                setActiveCategory('all');
              }}
            >
              {t('common.clear', 'Clear')}
            </Button>
          )}
        </div>
      </div>

      {filteredPresets.length === 0 ? (
        <div className="border border-dashed rounded-[1.5rem] bg-muted/5 py-10 px-6 text-center text-sm text-muted-foreground">
          {t(
            'mcpServer.noMatchingRecommended',
            'No recommended extensions match the current filters.',
          )}
        </div>
      ) : shouldGroupByCategory ? (
        <div className="space-y-8">
          {PRESET_CATEGORY_ORDER.map((category) => {
            const categoryPresets = groupedFilteredPresets[category];
            if (categoryPresets.length === 0) {
              return null;
            }

            const categoryMeta = PRESET_CATEGORY_META[category];
            const CategoryIcon = categoryMeta.icon;

            return (
              <section key={category} className="space-y-3">
                <div className="flex items-center gap-3">
                  <div className="flex items-center justify-center rounded-lg bg-primary/10 text-primary p-2">
                    <CategoryIcon className="w-4 h-4" />
                  </div>
                  <div>
                    <h4 className="font-semibold tracking-tight">
                      {t(categoryMeta.labelKey, categoryMeta.defaultLabel)}
                    </h4>
                    <p className="text-xs text-muted-foreground">
                      {t(
                        categoryMeta.descriptionKey,
                        categoryMeta.defaultDescription,
                      )}
                    </p>
                  </div>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {categoryPresets.map(renderPresetCard)}
                </div>
              </section>
            );
          })}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredPresets.map(renderPresetCard)}
        </div>
      )}
    </div>
  );
};
