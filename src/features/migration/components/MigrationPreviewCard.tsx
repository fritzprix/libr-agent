import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowDownToLine, AlertTriangle } from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Badge,
} from '@/components/ui';
import type {
  ConflictStrategy,
  MigrationPreview,
} from '@/lib/backend/migration';
import {
  formatBytes,
  getCompatibilityBadgeVariant,
  getCompatibilityKind,
  getCompatibilityWarningMessage,
  isIncompatible,
} from '../migration-utils';

export interface MigrationPreviewCardProps {
  preview: MigrationPreview;
  strategy: ConflictStrategy;
  setStrategy: (strategy: ConflictStrategy) => void;
  onImport: () => Promise<void>;
  onReset: () => void;
}

export const MigrationPreviewCard: FC<MigrationPreviewCardProps> = ({
  preview,
  strategy,
  setStrategy,
  onImport,
  onReset,
}) => {
  const { t } = useTranslation('common');
  const compatibilityKind = getCompatibilityKind(preview.compatibility);
  const warningMessage = getCompatibilityWarningMessage(preview.compatibility);

  return (
    <Card className="border border-border/80 shadow-lg bg-card/40 backdrop-blur-md rounded-2xl animate-in fade-in slide-in-from-bottom-4 duration-300">
      <CardHeader className="gap-2">
        <div className="flex justify-between items-start gap-4">
          <div>
            <CardTitle className="text-lg">
              {t(
                'settings.migration.preview.analysisComplete',
                'Migration archive analysis complete',
              )}
            </CardTitle>
            <CardDescription className="truncate max-w-lg">
              {preview.file_path}
            </CardDescription>
          </div>
          <Badge
            variant={getCompatibilityBadgeVariant(preview.compatibility)}
            className="rounded-lg py-1 px-2.5 text-xs font-semibold"
          >
            {compatibilityKind === 'compatible' &&
              t('settings.migration.preview.compatible', '✅ Compatible')}
            {compatibilityKind === 'newer' &&
              t(
                'settings.migration.preview.newerVersionWarning',
                '⚠️ Newer version warning',
              )}
            {compatibilityKind === 'incompatible' &&
              t('settings.migration.preview.incompatible', '❌ Incompatible')}
          </Badge>
        </div>
      </CardHeader>

      <CardContent className="flex flex-col gap-6">
        {warningMessage !== null && (
          <div className="p-4 rounded-xl border border-warning/20 bg-warning/5 flex gap-3 text-sm text-warning-foreground">
            <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
            <div>
              <span className="font-bold">
                {t(
                  'settings.migration.preview.compatibilityInfo',
                  'Compatibility info:',
                )}
              </span>{' '}
              {warningMessage}
            </div>
          </div>
        )}

        <div className="grid grid-cols-3 gap-4 text-center rounded-xl bg-background/30 p-4 border">
          <div>
            <div className="text-[10px] font-semibold text-muted-foreground uppercase">
              {t(
                'settings.migration.preview.backupAppVersion',
                'Backup app version',
              )}
            </div>
            <div className="text-sm font-bold text-foreground mt-1">
              {preview.app_version ||
                t('settings.migration.preview.unknown', 'Unknown')}
            </div>
          </div>
          <div>
            <div className="text-[10px] font-semibold text-muted-foreground uppercase">
              {t('settings.migration.preview.exportedAt', 'Exported at')}
            </div>
            <div className="text-sm font-bold text-foreground mt-1 truncate">
              {preview.exported_at
                ? new Date(preview.exported_at).toLocaleDateString()
                : t('settings.migration.preview.unknown', 'Unknown')}
            </div>
          </div>
          <div>
            <div className="text-[10px] font-semibold text-muted-foreground uppercase">
              {t('settings.migration.preview.totalSize', 'Total backup size')}
            </div>
            <div className="text-sm font-bold text-foreground mt-1">
              {formatBytes(preview.total_size_bytes)}
            </div>
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs font-semibold text-muted-foreground">
            {t(
              'settings.migration.preview.sectionsList',
              'Included backup sections',
            )}
          </span>
          <div className="rounded-xl border overflow-hidden bg-background/20">
            <table className="min-w-full divide-y divide-border">
              <thead className="bg-muted/40">
                <tr>
                  <th className="px-4 py-2 text-left text-[10px] font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.preview.sectionName', 'Section')}
                  </th>
                  <th className="px-4 py-2 text-center text-[10px] font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.preview.itemCount', 'Items')}
                  </th>
                  <th className="px-4 py-2 text-right text-[10px] font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.preview.fileSize', 'File size')}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border text-xs text-foreground/80">
                {preview.sections.map((sec) => (
                  <tr key={sec.name} className="hover:bg-background/25">
                    <td className="px-4 py-2 font-medium capitalize">
                      {sec.name.replace('_', ' ')}
                    </td>
                    <td className="px-4 py-2 text-center font-semibold">
                      {t('settings.migration.preview.itemCountValue', {
                        defaultValue: '{{count}} items',
                        count: sec.item_count,
                      })}
                    </td>
                    <td className="px-4 py-2 text-right text-muted-foreground">
                      {formatBytes(sec.size_bytes)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <div className="flex flex-col gap-3">
          <span className="text-xs font-semibold text-muted-foreground">
            {t(
              'settings.migration.preview.conflictStrategyLabel',
              'Import conflict resolution',
            )}
          </span>
          <div className="grid grid-cols-3 gap-3">
            <div
              onClick={() => setStrategy('skip')}
              className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                strategy === 'skip'
                  ? 'border-primary bg-primary/5 ring-1 ring-primary'
                  : 'bg-background/20'
              }`}
            >
              <span className="text-xs font-bold">
                {t(
                  'settings.migration.strategy.skipTitle',
                  'Skip (keep existing)',
                )}
              </span>
              <span className="text-[10px] text-muted-foreground leading-relaxed">
                {t(
                  'settings.migration.strategy.skipDescription',
                  'When names or IDs conflict, preserve the existing settings on this device.',
                )}
              </span>
            </div>
            <div
              onClick={() => setStrategy('overwrite')}
              className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                strategy === 'overwrite'
                  ? 'border-primary bg-primary/5 ring-1 ring-primary'
                  : 'bg-background/20'
              }`}
            >
              <span className="text-xs font-bold text-destructive">
                {t('settings.migration.strategy.overwriteTitle', 'Overwrite')}
              </span>
              <span className="text-[10px] text-muted-foreground leading-relaxed font-medium">
                {t(
                  'settings.migration.strategy.overwriteDescription',
                  'Remove all existing local records and fully replace them with backup data.',
                )}
              </span>
            </div>
            <div
              onClick={() => setStrategy('merge')}
              className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                strategy === 'merge'
                  ? 'border-primary bg-primary/5 ring-1 ring-primary'
                  : 'bg-background/20'
              }`}
            >
              <span className="text-xs font-bold">
                {t('settings.migration.strategy.mergeTitle', 'Merge')}
              </span>
              <span className="text-[10px] text-muted-foreground leading-relaxed">
                {t(
                  'settings.migration.strategy.mergeDescription',
                  'Keep existing settings while injecting new items. For safety, non-settings data is handled as Skip.',
                )}
              </span>
            </div>
          </div>
        </div>
      </CardContent>

      <CardFooter className="flex justify-between gap-3 border-t pt-4 bg-muted/20">
        <Button variant="ghost" onClick={onReset} className="rounded-xl h-10">
          {t('settings.migration.preview.cancelAndBack', 'Cancel and go back')}
        </Button>
        <Button
          onClick={onImport}
          disabled={isIncompatible(preview.compatibility)}
          className="gap-2 rounded-xl h-10 shadow-sm"
        >
          <ArrowDownToLine className="h-4 w-4" />
          {t('settings.migration.import.runImport', 'Run Import')}
        </Button>
      </CardFooter>
    </Card>
  );
};
