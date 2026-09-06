import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { ConflictStrategy } from '@/lib/backend/migration';
import { useMigration } from '../useMigration';

export function useMigrationPage() {
  const { t } = useTranslation('common');
  const migration = useMigration();
  const {
    error,
    selectedFilePath,
    includeSensitiveData,
    doInspect,
    doExport,
    doImport,
    doReverifyMcp,
    reset,
  } = migration;

  const [strategy, setStrategy] = useState<ConflictStrategy>('skip');
  const [reverifying, setReverifying] = useState(false);

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
    reset();
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

  return {
    ...migration,
    strategy,
    setStrategy,
    reverifying,
    isExportPasswordOpen,
    setIsExportPasswordOpen,
    exportPassword,
    setExportPassword,
    exportPasswordConfirm,
    setExportPasswordConfirm,
    exportPasswordError,
    isImportPasswordOpen,
    importPassword,
    setImportPassword,
    importPasswordError,
    handleCancelImportPassword,
    handleImportPasswordSubmit,
    handleExport,
    handleExportWithPassword,
    handleImport,
    handleReverify,
  };
}
