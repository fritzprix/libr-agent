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
import { SkillMetadata } from '@/types/skills';

const logger = getLogger('SkillsContext');

interface SkillsContextType {
  skills: SkillMetadata[];
  isLoading: boolean;
  error: string | null;
  refreshSkills: () => Promise<void>;
}

const SkillsContext = createContext<SkillsContextType | undefined>(undefined);

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  // Start as true so toast never fires before the first fetch completes
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { value: settings, isLoading: settingsLoading } = useSettings();

  const fetchSkills = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    let path = settings.system?.skillsDirectory;

    // If no path is configured, use the default [AppData]/skills via Tauri command
    if (!path) {
      try {
        path = await invoke<string>('get_default_skills_directory');
      } catch (e) {
        const errMsg = e instanceof Error ? e.message : String(e);
        logger.error('Failed to get default skills directory', e);
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
