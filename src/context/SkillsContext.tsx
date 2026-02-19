import React, {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
} from 'react';
import { getLogger } from '@/lib/logger';
import { useSettings } from '@/hooks/use-settings';
import { invoke } from '@tauri-apps/api/core';
import { appDataDir, join } from '@tauri-apps/api/path';

const logger = getLogger('SkillsContext');

export interface SkillMetadata {
  name: string;
  description: string;
  path: string;
}

interface SkillsContextType {
  skills: SkillMetadata[];
  isLoading: boolean;
  error: string | null;
  refreshSkills: () => Promise<void>;
}

const SkillsContext = createContext<SkillsContextType | undefined>(undefined);

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { value: settings, isLoading: settingsLoading } = useSettings();

  const fetchSkills = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    let path = settings.system?.skillsDirectory;

    // If no path is configured, default to [AppData]/skills
    if (!path) {
      try {
        const dataDir = await appDataDir();
        path = await join(dataDir, 'skills');
      } catch (e) {
        const errMsg = e instanceof Error ? e.message : String(e);
        logger.error('Failed to get AppData dir for default skills path', e);
        setError(`Failed to determine skills directory: ${errMsg}`);
        setIsLoading(false);
        return;
      }
    }

    try {
      logger.info('Scanning skills directory:', path);
      const result = await invoke<SkillMetadata[]>('scan_skills_directory', {
        directory: path,
      });
      logger.info('Discovered skills:', {
        count: result.length,
        skills: result,
      });
      setSkills(result);
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err);
      logger.warn('Failed to scan skills directory', err);
      setError(`Failed to scan skills: ${errMsg}`);
      setSkills([]);
    } finally {
      setIsLoading(false);
    }
  }, [settings.system?.skillsDirectory]);

  // Initial fetch - wait for settings to load, then scan.
  // fetchSkills() already falls back to [AppData]/skills when skillsDirectory is not configured.
  useEffect(() => {
    if (!settingsLoading) {
      fetchSkills();
    }
  }, [fetchSkills, settingsLoading]);

  return (
    <SkillsContext.Provider
      value={{
        skills,
        isLoading,
        error,
        refreshSkills: fetchSkills,
      }}
    >
      {children}
    </SkillsContext.Provider>
  );
}

export function useSkills() {
  const context = useContext(SkillsContext);
  if (context === undefined) {
    throw new Error('useSkills must be used within a SkillsProvider');
  }
  return context;
}
