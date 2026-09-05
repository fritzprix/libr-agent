import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, FolderSync, RefreshCw } from 'lucide-react';
import { Button, Card, Separator } from '@/components/ui';
import type {
  MigrationExportInfo,
  MigrationImportResult,
} from '@/lib/backend/migration';
import { formatBytes } from '../migration-utils';

export interface MigrationCompleteCardProps {
  exportInfo: MigrationExportInfo | null;
  importResult: MigrationImportResult | null;
  reverifying: boolean;
  onReverify: () => Promise<void>;
  onReset: () => void;
}

export const MigrationCompleteCard: FC<MigrationCompleteCardProps> = ({
  exportInfo,
  importResult,
  reverifying,
  onReverify,
  onReset,
}) => {
  const { t } = useTranslation('common');

  return (
    <Card className="border border-border/80 shadow-lg bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center gap-6 animate-in fade-in duration-300">
      <div className="h-16 w-16 rounded-full bg-primary/10 text-primary flex items-center justify-center animate-bounce">
        <CheckCircle2 className="h-8 w-8" />
      </div>

      <div className="text-center">
        <h2 className="text-xl font-bold text-foreground">
          {exportInfo
            ? t('settings.migration.complete.exportTitle', 'Export complete!')
            : t(
                'settings.migration.complete.importTitle',
                'Import successful!',
              )}
        </h2>
        <p className="text-sm text-muted-foreground mt-2 max-w-md">
          {exportInfo
            ? t(
                'settings.migration.complete.exportDescription',
                'A backup file of the current environment settings was packaged and created successfully.',
              )
            : t(
                'settings.migration.complete.importDescription',
                'Imported settings passed integrity verification and were applied safely. Complete the cleanup below to activate the changes.',
              )}
        </p>
      </div>

      <Separator />

      {exportInfo && (
        <div className="w-full flex flex-col gap-2 text-xs p-4 rounded-xl border bg-background/30 text-left">
          <div className="flex justify-between">
            <span className="text-muted-foreground">
              {t('settings.migration.complete.savePath', 'Save path:')}
            </span>
            <span className="font-mono text-foreground font-semibold truncate max-w-sm">
              {exportInfo.file_path}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">
              {t('settings.migration.complete.fileSize', 'File size:')}
            </span>
            <span className="text-foreground font-semibold">
              {formatBytes(exportInfo.file_size_bytes)}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">
              {t(
                'settings.migration.complete.includedSections',
                'Included sections:',
              )}
            </span>
            <span className="text-foreground font-semibold">
              {exportInfo.sections.join(', ')}
            </span>
          </div>
        </div>
      )}

      {importResult && (
        <div className="w-full flex flex-col gap-4">
          <div className="rounded-xl border overflow-hidden bg-background/20 text-xs text-left">
            <table className="min-w-full divide-y divide-border">
              <thead className="bg-muted/40">
                <tr>
                  <th className="px-4 py-2 font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.complete.section', 'Section')}
                  </th>
                  <th className="px-4 py-2 text-center font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.complete.success', 'Success')}
                  </th>
                  <th className="px-4 py-2 text-center font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.complete.skipped', 'Skipped')}
                  </th>
                  <th className="px-4 py-2 text-right font-semibold text-muted-foreground uppercase">
                    {t('settings.migration.complete.errors', 'Errors')}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border text-foreground/80">
                {Object.entries(importResult.sections_imported).map(
                  ([name, report]) => (
                    <tr key={name} className="hover:bg-background/25">
                      <td className="px-4 py-2 font-medium capitalize">
                        {name.replace('_', ' ')}
                      </td>
                      <td className="px-4 py-2 text-center text-primary font-bold">
                        {report.success}
                      </td>
                      <td className="px-4 py-2 text-center text-muted-foreground font-semibold">
                        {report.skipped}
                      </td>
                      <td className="px-4 py-2 text-right text-destructive font-bold">
                        {report.errors.length}
                      </td>
                    </tr>
                  ),
                )}
              </tbody>
            </table>
          </div>

          <div className="p-4 rounded-xl border border-primary/20 bg-primary/5 flex flex-col gap-3 text-left">
            <div className="flex gap-2.5 items-start text-xs text-muted-foreground leading-relaxed">
              <FolderSync className="h-5 w-5 text-primary shrink-0" />
              <div>
                <span className="font-semibold text-foreground">
                  {t(
                    'settings.migration.complete.mcpRestoreLabel',
                    '🔌 MCP and background environment restore:',
                  )}
                </span>{' '}
                {t(
                  'settings.migration.complete.mcpRestoreDescription',
                  'After import, MCP services need token refresh and verification to run correctly in the new host/environment. Click the verify button below.',
                )}
              </div>
            </div>
            <Button
              onClick={onReverify}
              disabled={reverifying}
              className="w-full gap-2 rounded-lg h-9 shadow-sm"
            >
              <RefreshCw
                className={`h-4 w-4 ${reverifying ? 'animate-spin' : ''}`}
              />
              {t(
                'settings.migration.complete.mcpReverifyButton',
                'Re-authenticate and verify MCP servers',
              )}
            </Button>
          </div>
        </div>
      )}

      <Button
        onClick={onReset}
        className="w-full rounded-xl h-10 font-bold"
        variant="outline"
      >
        {t('settings.migration.complete.backToMain', 'Back to migration home')}
      </Button>
    </Card>
  );
};
