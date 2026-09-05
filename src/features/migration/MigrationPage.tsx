import type { FC } from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';
import {
  ArrowLeft,
  ArrowUpFromLine,
  ArrowDownToLine,
  ShieldAlert,
  CheckCircle2,
  FolderOpen,
  FileCheck2,
  RefreshCw,
  Info,
  AlertTriangle,
  FolderSync,
} from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Badge,
  Separator,
} from '@/components/ui';
import { useMigration } from './useMigration';
import type { ConflictStrategy } from '@/lib/backend/migration';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { useEffect } from 'react';

const MigrationPage: FC = () => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();

  const {
    phase,
    preview,
    exportInfo,
    importResult,
    error,
    progress,
    selectedFilePath,
    selectedExportDir,
    includeSensitiveData,
    setIncludeSensitiveData,
    selectExportFile,
    selectImportFile,
    doInspect,
    doExport,
    doImport,
    doReverifyMcp,
    reset,
  } = useMigration();

  const [strategy, setStrategy] = useState<ConflictStrategy>('skip');
  const [reverifying, setReverifying] = useState(false);

  // Password-based backup states
  const [isExportPasswordOpen, setIsExportPasswordOpen] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportPasswordConfirm, setExportPasswordConfirm] = useState('');
  const [exportPasswordError, setExportPasswordError] = useState<string | null>(
    null,
  );

  const [isImportPasswordOpen, setIsImportPasswordOpen] = useState(false);
  const [importPassword, setImportPassword] = useState('');
  const [importPasswordError, setImportPasswordError] = useState<string | null>(
    null,
  );
  const [currentPassword, setCurrentPassword] = useState<string | undefined>(
    undefined,
  );

  // Monitor for password prompt error from the backend
  useEffect(() => {
    if (error === 'PASSWORD_REQUIRED') {
      setIsImportPasswordOpen(true);
      setImportPasswordError(null);
    } else if (error === 'WRONG_PASSWORD') {
      setIsImportPasswordOpen(true);
      setImportPasswordError(
        t(
          'settings.migration.errors.wrongPassword',
          'Incorrect password. Please try again.',
        ),
      );
    }
  }, [error, t]);

  const handleCancelImportPassword = () => {
    setIsImportPasswordOpen(false);
    setImportPassword('');
    setImportPasswordError(null);
    reset(); // Reset useMigration state to clear password error
  };

  const handleImportPasswordSubmit = async () => {
    if (!selectedFilePath) return;
    try {
      setImportPasswordError(null);
      await doInspect(selectedFilePath, importPassword);
      setCurrentPassword(importPassword);
      setIsImportPasswordOpen(false);
      setImportPassword('');
      toast.success(
        t(
          'settings.migration.toasts.backupPasswordVerified',
          'Backup file password verified',
        ),
      );
    } catch (e) {
      if (e instanceof Error && e.message === 'WRONG_PASSWORD') {
        setImportPasswordError(
          t(
            'settings.migration.errors.wrongPassword',
            'Incorrect password. Please try again.',
          ),
        );
      } else {
        setIsImportPasswordOpen(false);
        setImportPassword('');
      }
    }
  };

  const handleExport = async () => {
    if (includeSensitiveData) {
      setExportPassword('');
      setExportPasswordConfirm('');
      setExportPasswordError(null);
      setIsExportPasswordOpen(true);
    } else {
      try {
        await doExport();
        toast.success(
          t(
            'settings.migration.toasts.exportSuccess',
            'Data exported successfully!',
          ),
        );
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        toast.error(
          t('settings.migration.toasts.exportFailed', {
            defaultValue: 'Export failed: {{error}}',
            error: msg,
          }),
        );
      }
    }
  };

  const handleExportWithPassword = async () => {
    if (exportPassword.length < 4) {
      setExportPasswordError(
        t(
          'settings.migration.errors.passwordMinLength',
          'Password must be at least 4 characters.',
        ),
      );
      return;
    }
    if (exportPassword !== exportPasswordConfirm) {
      setExportPasswordError(
        t(
          'settings.migration.errors.passwordMismatch',
          'Passwords do not match.',
        ),
      );
      return;
    }

    try {
      setExportPasswordError(null);
      await doExport(exportPassword);
      setIsExportPasswordOpen(false);
      toast.success(
        t(
          'settings.migration.toasts.exportEncryptedSuccess',
          'Encrypted data exported successfully!',
        ),
      );
    } catch (e) {
      setIsExportPasswordOpen(false);
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(
        t('settings.migration.toasts.exportFailed', {
          defaultValue: 'Export failed: {{error}}',
          error: msg,
        }),
      );
    }
  };

  const handleImport = async () => {
    try {
      await doImport(strategy, currentPassword);
      toast.success(
        t(
          'settings.migration.toasts.importSuccess',
          'Migration data imported successfully!',
        ),
      );
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(
        t('settings.migration.toasts.importFailed', {
          defaultValue: 'Import failed: {{error}}',
          error: msg,
        }),
      );
    }
  };

  const handleReverify = async () => {
    setReverifying(true);
    try {
      const results = await doReverifyMcp();
      const failedServers = Object.entries(results)
        .filter(([, status]) => status === 'error')
        .map(([id]) => id);

      if (failedServers.length > 0) {
        toast.warning(
          t('settings.migration.toasts.mcpReverifyPartial', {
            defaultValue:
              'Some MCP servers failed re-verification: {{servers}}',
            servers: failedServers.join(', '),
          }),
        );
      } else {
        toast.success(
          t(
            'settings.migration.toasts.mcpReverifySuccess',
            'MCP server re-verification completed.',
          ),
        );
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(
        t('settings.migration.toasts.reverifyFailed', {
          defaultValue: 'Re-verification failed: {{error}}',
          error: msg,
        }),
      );
    } finally {
      setReverifying(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="p-6 min-h-full bg-background/50 flex flex-col items-center">
      <div className="max-w-3xl w-full flex flex-col gap-6">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => navigate('/settings')}
            className="h-10 w-10 rounded-xl"
          >
            <ArrowLeft className="h-5 w-5 text-muted-foreground" />
          </Button>
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              {t('settings.migration.title', 'Data Migration')}
            </h1>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.migration.subtitle',
                'Transfer LibrAgent settings, assistants, scheduler, and custom skills to another device.',
              )}
            </p>
          </div>
        </div>

        {error &&
          error !== 'PASSWORD_REQUIRED' &&
          error !== 'WRONG_PASSWORD' && (
            <div className="p-4 rounded-xl border border-destructive/20 bg-destructive/10 text-destructive-foreground flex gap-3 items-start animate-in fade-in slide-in-from-top-4 duration-300">
              <ShieldAlert className="h-5 w-5 shrink-0 text-destructive mt-0.5" />
              <div className="text-sm flex-1">
                <span className="font-semibold">
                  {t('settings.migration.errors.occurred', 'Error occurred:')}
                </span>{' '}
                {error}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={reset}
                className="h-7 text-xs hover:bg-destructive/20 text-destructive-foreground"
              >
                {t('settings.migration.errors.reset', 'Reset')}
              </Button>
            </div>
          )}

        {phase === 'idle' && !preview && !exportInfo && !importResult && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Export Card */}
            <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
              <CardHeader className="gap-2">
                <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                  <ArrowUpFromLine className="h-6 w-6" />
                </div>
                <CardTitle className="text-lg">
                  {t('settings.migration.export.title', 'Export Settings')}
                </CardTitle>
                <CardDescription>
                  {t(
                    'settings.migration.export.description',
                    'Package all settings and user skill data from this device into a single archive file (`.libragent-migration`).',
                  )}
                </CardDescription>
              </CardHeader>

              <CardContent className="flex-1 flex flex-col gap-4">
                <div className="rounded-xl border border-warning/20 bg-warning/5 p-4 flex gap-3 items-start">
                  <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
                  <div className="text-xs text-muted-foreground leading-relaxed">
                    <span className="font-semibold text-foreground">
                      {t(
                        'settings.migration.export.securityWarningLabel',
                        '⚠️ Security notice:',
                      )}
                    </span>{' '}
                    {t(
                      'settings.migration.export.securityWarning',
                      'API keys and access tokens are masked by default. Enable the option below only when moving them through another secure channel.',
                    )}
                  </div>
                </div>

                <div
                  className="flex items-center gap-3 p-3 rounded-xl border bg-background/50 hover:bg-background/80 transition-colors duration-200 cursor-pointer"
                  onClick={() => setIncludeSensitiveData(!includeSensitiveData)}
                >
                  <input
                    type="checkbox"
                    checked={includeSensitiveData}
                    onChange={(e) => setIncludeSensitiveData(e.target.checked)}
                    className="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary cursor-pointer"
                  />
                  <div className="flex flex-col">
                    <span className="text-xs font-semibold text-foreground">
                      {t(
                        'settings.migration.export.includeSensitiveLabel',
                        'Include sensitive data (full Settings table)',
                      )}
                    </span>
                    <span className="text-[10px] text-muted-foreground">
                      {t(
                        'settings.migration.export.includeSensitiveDescription',
                        'Store plaintext data from the entire settings table—including API keys and access tokens—in the backup archive.',
                      )}
                    </span>
                  </div>
                </div>

                <div className="flex flex-col gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    {t(
                      'settings.migration.export.selectPathLabel',
                      'Select export path',
                    )}
                  </span>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      readOnly
                      placeholder={t(
                        'settings.migration.export.selectPathPlaceholder',
                        'Select a folder to export to...',
                      )}
                      value={selectedExportDir || ''}
                      className="flex-1 px-3 py-2 text-xs rounded-lg border bg-background/30 focus:outline-none truncate"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={selectExportFile}
                      className="gap-1.5 h-9 rounded-lg"
                    >
                      <FolderOpen className="h-4 w-4" />
                      {t('settings.migration.export.browse', 'Browse')}
                    </Button>
                  </div>
                </div>
              </CardContent>

              <CardFooter className="pt-2">
                <Button
                  onClick={handleExport}
                  disabled={!selectedExportDir}
                  className="w-full gap-2 rounded-xl h-10 shadow-sm"
                >
                  <ArrowUpFromLine className="h-4 w-4" />
                  {t('settings.migration.export.runExport', 'Run Export')}
                </Button>
              </CardFooter>
            </Card>

            {/* Import Card */}
            <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
              <CardHeader className="gap-2">
                <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                  <ArrowDownToLine className="h-6 w-6" />
                </div>
                <CardTitle className="text-lg">
                  {t('settings.migration.import.title', 'Import Settings')}
                </CardTitle>
                <CardDescription>
                  {t(
                    'settings.migration.import.description',
                    'Load a previously exported `.libragent-migration` archive file to update the current environment.',
                  )}
                </CardDescription>
              </CardHeader>

              <CardContent className="flex-1 flex flex-col justify-between">
                <div className="rounded-xl border border-primary/20 bg-primary/5 p-4 flex gap-3 items-start mb-6">
                  <Info className="h-5 w-5 shrink-0 text-primary mt-0.5" />
                  <div className="text-xs text-muted-foreground leading-relaxed">
                    {t(
                      'settings.migration.import.autoBackupInfo',
                      'Before importing, an automatic backup of the current database is created to prevent data loss. If import fails, the previous state is restored immediately.',
                    )}
                  </div>
                </div>

                <div className="flex flex-col gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    {t(
                      'settings.migration.import.archiveFileLabel',
                      'Migration archive file',
                    )}
                  </span>
                  <Button
                    variant="outline"
                    onClick={selectImportFile}
                    className="w-full gap-2 h-14 border-dashed border-2 hover:border-primary/50 hover:bg-primary/5 rounded-xl transition-all duration-200"
                  >
                    <FileCheck2 className="h-5 w-5 text-muted-foreground" />
                    {t(
                      'settings.migration.import.selectAndValidate',
                      'Select file and validate',
                    )}
                  </Button>
                </div>
              </CardContent>

              <CardFooter className="pt-2">
                <Button disabled className="w-full gap-2 rounded-xl h-10">
                  <ArrowDownToLine className="h-4 w-4" />
                  {t(
                    'settings.migration.import.runImportWaiting',
                    'Run Import (awaiting file analysis)',
                  )}
                </Button>
              </CardFooter>
            </Card>
          </div>
        )}

        {/* Loading Phases (inspecting, exporting, importing) */}
        {(phase === 'inspecting' ||
          phase === 'exporting' ||
          phase === 'importing' ||
          phase === 'selecting') && (
          <Card className="border border-border/80 shadow-md bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center justify-center gap-6 text-center animate-in fade-in duration-300">
            <div className="relative flex items-center justify-center">
              <RefreshCw className="h-12 w-12 text-primary animate-spin" />
              <span className="absolute text-[10px] font-bold text-primary">
                {progress}%
              </span>
            </div>
            <div>
              <h3 className="text-lg font-bold text-foreground capitalize">
                {phase === 'inspecting' &&
                  t(
                    'settings.migration.loading.inspectingTitle',
                    'Analyzing file structure...',
                  )}
                {phase === 'exporting' &&
                  t(
                    'settings.migration.loading.exportingTitle',
                    'Writing migration file...',
                  )}
                {phase === 'importing' &&
                  t(
                    'settings.migration.loading.importingTitle',
                    'Applying to environment database...',
                  )}
                {phase === 'selecting' &&
                  t(
                    'settings.migration.loading.selectingTitle',
                    'Waiting for user input...',
                  )}
              </h3>
              <p className="text-sm text-muted-foreground mt-2 max-w-sm">
                {phase === 'inspecting' &&
                  t(
                    'settings.migration.loading.inspectingDescription',
                    'Inspecting contents and performing threat analysis for ZIP Slip protection.',
                  )}
                {phase === 'exporting' &&
                  t(
                    'settings.migration.loading.exportingDescription',
                    'Serializing settings tables and archiving skill files.',
                  )}
                {phase === 'importing' &&
                  t(
                    'settings.migration.loading.importingDescription',
                    'Applying settings within an isolated transaction and validating foreign key constraints.',
                  )}
                {phase === 'selecting' &&
                  t(
                    'settings.migration.loading.selectingDescription',
                    'Complete your selection in the folder/file explorer window.',
                  )}
              </p>
            </div>
            <div className="w-full max-w-xs bg-muted rounded-full h-1.5 overflow-hidden">
              <div
                className="bg-primary h-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              ></div>
            </div>
          </Card>
        )}

        {/* Inspect Preview Page */}
        {phase === 'idle' && preview && !importResult && (
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
                  variant={
                    preview.compatibility === 'Compatible'
                      ? 'default'
                      : typeof preview.compatibility === 'object' &&
                          'NewerVersion' in preview.compatibility
                        ? 'secondary'
                        : 'destructive'
                  }
                  className="rounded-lg py-1 px-2.5 text-xs font-semibold"
                >
                  {preview.compatibility === 'Compatible' &&
                    t('settings.migration.preview.compatible', '✅ Compatible')}
                  {typeof preview.compatibility === 'object' &&
                    'NewerVersion' in preview.compatibility &&
                    t(
                      'settings.migration.preview.newerVersionWarning',
                      '⚠️ Newer version warning',
                    )}
                  {typeof preview.compatibility === 'object' &&
                    'Incompatible' in preview.compatibility &&
                    t(
                      'settings.migration.preview.incompatible',
                      '❌ Incompatible',
                    )}
                </Badge>
              </div>
            </CardHeader>

            <CardContent className="flex flex-col gap-6">
              {/* Compatibility warning if newer version or incompatible */}
              {typeof preview.compatibility === 'object' && (
                <div className="p-4 rounded-xl border border-warning/20 bg-warning/5 flex gap-3 text-sm text-warning-foreground">
                  <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
                  <div>
                    <span className="font-bold">
                      {t(
                        'settings.migration.preview.compatibilityInfo',
                        'Compatibility info:',
                      )}
                    </span>{' '}
                    {'NewerVersion' in preview.compatibility
                      ? preview.compatibility.NewerVersion.message
                      : 'Incompatible' in preview.compatibility
                        ? preview.compatibility.Incompatible.message
                        : ''}
                  </div>
                </div>
              )}

              {/* Archive Info */}
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
                    {t(
                      'settings.migration.preview.totalSize',
                      'Total backup size',
                    )}
                  </div>
                  <div className="text-sm font-bold text-foreground mt-1">
                    {formatBytes(preview.total_size_bytes)}
                  </div>
                </div>
              </div>

              {/* Sections List */}
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
                          {t(
                            'settings.migration.preview.sectionName',
                            'Section',
                          )}
                        </th>
                        <th className="px-4 py-2 text-center text-[10px] font-semibold text-muted-foreground uppercase">
                          {t('settings.migration.preview.itemCount', 'Items')}
                        </th>
                        <th className="px-4 py-2 text-right text-[10px] font-semibold text-muted-foreground uppercase">
                          {t(
                            'settings.migration.preview.fileSize',
                            'File size',
                          )}
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

              {/* Strategy Selection */}
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
                      {t(
                        'settings.migration.strategy.overwriteTitle',
                        'Overwrite',
                      )}
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
              <Button
                variant="ghost"
                onClick={reset}
                className="rounded-xl h-10"
              >
                {t(
                  'settings.migration.preview.cancelAndBack',
                  'Cancel and go back',
                )}
              </Button>
              <Button
                onClick={handleImport}
                disabled={
                  typeof preview.compatibility === 'object' &&
                  'Incompatible' in preview.compatibility
                }
                className="gap-2 rounded-xl h-10 shadow-sm"
              >
                <ArrowDownToLine className="h-4 w-4" />
                {t('settings.migration.import.runImport', 'Run Import')}
              </Button>
            </CardFooter>
          </Card>
        )}

        {/* Complete Result Screen */}
        {phase === 'complete' && (exportInfo || importResult) && (
          <Card className="border border-border/80 shadow-lg bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center gap-6 animate-in fade-in duration-300">
            <div className="h-16 w-16 rounded-full bg-primary/10 text-primary flex items-center justify-center animate-bounce">
              <CheckCircle2 className="h-8 w-8" />
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-foreground">
                {exportInfo
                  ? t(
                      'settings.migration.complete.exportTitle',
                      'Export complete!',
                    )
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
                    onClick={handleReverify}
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
              onClick={reset}
              className="w-full rounded-xl h-10 font-bold"
              variant="outline"
            >
              {t(
                'settings.migration.complete.backToMain',
                'Back to migration home',
              )}
            </Button>
          </Card>
        )}
      </div>

      {/* Export Password Modal */}
      <Dialog
        open={isExportPasswordOpen}
        onOpenChange={setIsExportPasswordOpen}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              {t(
                'settings.migration.exportPassword.title',
                'Set backup security password',
              )}
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              {t(
                'settings.migration.exportPassword.description',
                'Because this backup includes sensitive data (API keys, credentials, etc.), you must set a password to encrypt the backup file. This password is required when restoring (importing) the backup later.',
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.exportPassword.passwordLabel',
                  'Password (minimum 4 characters)',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.exportPassword.passwordPlaceholder',
                  'Enter password',
                )}
                value={exportPassword}
                onChange={(e) => setExportPassword(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.exportPassword.confirmLabel',
                  'Confirm password',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.exportPassword.confirmPlaceholder',
                  'Re-enter password',
                )}
                value={exportPasswordConfirm}
                onChange={(e) => setExportPasswordConfirm(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {exportPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {exportPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setIsExportPasswordOpen(false)}
              className="rounded-lg h-9"
            >
              {t('settings.migration.exportPassword.cancel', 'Cancel')}
            </Button>
            <Button
              size="sm"
              onClick={handleExportWithPassword}
              className="rounded-lg h-9 font-semibold"
            >
              {t('settings.migration.exportPassword.runExport', 'Run Export')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Import Password Modal */}
      <Dialog
        open={isImportPasswordOpen}
        onOpenChange={(open) => {
          if (!open) handleCancelImportPassword();
        }}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              {t(
                'settings.migration.importPassword.title',
                'Enter security password',
              )}
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              {t(
                'settings.migration.importPassword.description',
                'This backup file is encrypted. A password is required to preview and import it. Enter the password you set when exporting.',
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.importPassword.passwordLabel',
                  'Password',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.importPassword.passwordPlaceholder',
                  'Enter password',
                )}
                value={importPassword}
                onChange={(e) => setImportPassword(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleImportPasswordSubmit();
                }}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {importPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {importPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={handleCancelImportPassword}
              className="rounded-lg h-9"
            >
              {t('settings.migration.importPassword.cancel', 'Cancel')}
            </Button>
            <Button
              size="sm"
              onClick={handleImportPasswordSubmit}
              className="rounded-lg h-9 font-semibold"
            >
              {t('settings.migration.importPassword.confirm', 'Confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
};

export default MigrationPage;
