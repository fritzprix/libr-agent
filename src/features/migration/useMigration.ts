import { useState, useCallback, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import {
  exportMigration,
  importMigration,
  inspectMigration,
  reverifyMcpServers,
  type MigrationPreview,
  type MigrationExportInfo,
  type MigrationImportResult,
  type ConflictStrategy,
} from '@/lib/backend/migration';

export type MigrationPhase =
  | 'idle'
  | 'selecting'
  | 'inspecting'
  | 'importing'
  | 'exporting'
  | 'complete'
  | 'error';

export interface UseMigrationReturn {
  phase: MigrationPhase;
  preview: MigrationPreview | null;
  exportInfo: MigrationExportInfo | null;
  importResult: MigrationImportResult | null;
  error: string | null;
  progress: number; // 0-100 progress indicator
  selectedFilePath: string | null;
  selectedExportDir: string | null;
  includeSensitiveData: boolean;
  setIncludeSensitiveData: (val: boolean) => void;
  selectExportFile: () => Promise<void>;
  selectImportFile: () => Promise<void>;
  doInspect: (filePath: string, password?: string) => Promise<void>;
  doExport: (password?: string) => Promise<void>;
  doImport: (strategy: ConflictStrategy, password?: string) => Promise<void>;
  doReverifyMcp: () => Promise<Record<string, 'success' | 'error' | 'skipped'>>;
  reset: () => void;
}

export function useMigration(): UseMigrationReturn {
  const [phase, setPhase] = useState<MigrationPhase>('idle');
  const [preview, setPreview] = useState<MigrationPreview | null>(null);
  const [exportInfo, setExportInfo] = useState<MigrationExportInfo | null>(
    null,
  );
  const [importResult, setImportResult] =
    useState<MigrationImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [selectedExportDir, setSelectedExportDir] = useState<string | null>(
    null,
  );
  const [includeSensitiveData, setIncludeSensitiveData] =
    useState<boolean>(false);

  // Subscribe to tauri progress updates
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<number>('migration:progress', (event) => {
          setProgress(event.payload);
        });
      } catch (err) {
        console.error('Failed to subscribe to migration progress events', err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const reset = useCallback(() => {
    setPhase('idle');
    setPreview(null);
    setExportInfo(null);
    setImportResult(null);
    setError(null);
    setProgress(0);
    setSelectedFilePath(null);
    setSelectedExportDir(null);
    setIncludeSensitiveData(false);
  }, []);

  const selectExportFile = useCallback(async () => {
    setPhase('selecting');
    setError(null);
    try {
      const selected = await open({
        title: '내보낼 폴더 선택',
        directory: true,
        multiple: false,
      });
      if (selected) {
        const path = typeof selected === 'string' ? selected : selected[0];
        if (path) {
          setSelectedExportDir(path);
        }
      }
      setPhase('idle');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase('idle');
    }
  }, []);

  const doInspect = useCallback(async (filePath: string, password?: string) => {
    setPhase('inspecting');
    setProgress(20);
    setError(null);
    try {
      const result = await inspectMigration(filePath, password);
      setPreview(result);
      setSelectedFilePath(filePath);
      setProgress(100);
      setPhase('idle');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase('error');
      throw e;
    }
  }, []);

  const selectImportFile = useCallback(async () => {
    setPhase('selecting');
    setError(null);
    try {
      const selected = await open({
        title: '마이그레이션 파일 선택',
        filters: [
          { name: 'LibrAgent Migration', extensions: ['libragent-migration'] },
        ],
        multiple: false,
      });
      if (selected) {
        const path = typeof selected === 'string' ? selected : selected[0];
        if (path) {
          setSelectedFilePath(path);
          await doInspect(path);
        } else {
          setPhase('idle');
        }
      } else {
        setPhase('idle');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase('idle');
    }
  }, [doInspect]);

  const doExport = useCallback(
    async (password?: string) => {
      if (!selectedExportDir) {
        setError('저장할 폴더가 선택되지 않았습니다.');
        return;
      }
      setPhase('exporting');
      setProgress(30);
      setError(null);
      try {
        // Create output filename with timestamp
        const timestamp = new Date()
          .toISOString()
          .replace(/[:.]/g, '-')
          .substring(0, 19);
        const fileName = `libragent-backup-${timestamp}.libragent-migration`;

        // Handle slash replacement depending on platform
        const separator = selectedExportDir.includes('\\') ? '\\' : '/';
        const fullPath = selectedExportDir.endsWith(separator)
          ? `${selectedExportDir}${fileName}`
          : `${selectedExportDir}${separator}${fileName}`;

        const result = await exportMigration(
          fullPath,
          includeSensitiveData,
          password,
        );
        setExportInfo(result);
        setProgress(100);
        setPhase('complete');
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setPhase('error');
        throw e;
      }
    },
    [selectedExportDir, includeSensitiveData],
  );

  const doImport = useCallback(
    async (strategy: ConflictStrategy, password?: string) => {
      if (!selectedFilePath) return;
      setPhase('importing');
      setProgress(40);
      setError(null);
      try {
        const result = await importMigration(
          selectedFilePath,
          strategy,
          password,
        );
        setImportResult(result);
        setProgress(100);
        setPhase('complete');
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setPhase('error');
        throw e;
      }
    },
    [selectedFilePath],
  );

  const doReverifyMcp = useCallback(async (): Promise<
    Record<string, 'success' | 'error' | 'skipped'>
  > => {
    try {
      return await reverifyMcpServers();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e;
    }
  }, []);

  return {
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
  };
}
