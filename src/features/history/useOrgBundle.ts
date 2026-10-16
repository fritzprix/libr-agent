import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import {
  exportOrgBundle,
  importOrgBundle,
  inspectOrgBundle,
  type OrgBundleExportInfo,
  type OrgBundleImportResult,
} from '@/lib/backend/org-bundle';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useOrgBundle');

export function useOrgBundle() {
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [lastExport, setLastExport] = useState<OrgBundleExportInfo | null>(
    null,
  );
  const [lastImport, setLastImport] = useState<OrgBundleImportResult | null>(
    null,
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<number>('org-bundle:progress', (event) => {
      setProgress(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const exportOrg = useCallback(async (orgId: string, orgName: string) => {
    setBusy(true);
    setProgress(0);
    setLastExport(null);
    try {
      const selected = await open({
        title: 'Export Org bundle folder',
        directory: true,
        multiple: false,
      });
      if (!selected) {
        return null;
      }
      const dir = typeof selected === 'string' ? selected : selected[0];
      if (!dir) {
        return null;
      }

      const safeName = orgName.replace(/[^\w-]+/g, '_').slice(0, 48) || 'org';
      const timestamp = new Date()
        .toISOString()
        .replace(/[:.]/g, '-')
        .substring(0, 19);
      const fileName = `libragent-org-${safeName}-${timestamp}.libragent-org`;
      const separator = dir.includes('\\') ? '\\' : '/';
      const fullPath = dir.endsWith(separator)
        ? `${dir}${fileName}`
        : `${dir}${separator}${fileName}`;

      const result = await exportOrgBundle(orgId, fullPath, false);
      setLastExport(result);
      return result;
    } catch (error) {
      logger.error('Failed to export org bundle', error);
      throw error;
    } finally {
      setBusy(false);
    }
  }, []);

  const importOrg = useCallback(async () => {
    setBusy(true);
    setProgress(0);
    setLastImport(null);
    try {
      const selected = await open({
        title: 'Import Org bundle',
        filters: [
          { name: 'LibrAgent Org Bundle', extensions: ['libragent-org'] },
        ],
        multiple: false,
      });
      if (!selected) {
        return null;
      }
      const filePath = typeof selected === 'string' ? selected : selected[0];
      if (!filePath) {
        return null;
      }

      // Validate kind before import (throws if wrong kind / password required)
      await inspectOrgBundle(filePath);
      const result = await importOrgBundle(filePath, 'merge');
      setLastImport(result);
      return result;
    } catch (error) {
      logger.error('Failed to import org bundle', error);
      throw error;
    } finally {
      setBusy(false);
    }
  }, []);

  return {
    busy,
    progress,
    lastExport,
    lastImport,
    exportOrg,
    importOrg,
  };
}
