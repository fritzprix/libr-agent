import { safeInvoke } from '@/lib/backend/core';
import React, {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
} from 'react';
import { getLogger } from '@/lib/logger';
import { useSettings } from '@/hooks/use-settings';

import { SkillMetadata } from '@/types/skills';

const logger = getLogger('SkillsContext');

let cachedManagedSkills: SkillMetadata[] | null = null;
let cachedManagedSkillsPromise: Promise<SkillMetadata[]> | null = null;

async function loadManagedSkills(
  forceRefresh = false,
): Promise<SkillMetadata[]> {
  if (!forceRefresh && cachedManagedSkills !== null) {
    return cachedManagedSkills;
  }

  if (!forceRefresh && cachedManagedSkillsPromise) {
    return cachedManagedSkillsPromise;
  }

  const request = safeInvoke<{ effectiveSkills: SkillMetadata[] }>(
    'get_managed_skills_overview',
  )
    .then((overview) => {
      const result = overview.effectiveSkills ?? [];
      cachedManagedSkills = result;
      return result;
    })
    .finally(() => {
      if (cachedManagedSkillsPromise === request) {
        cachedManagedSkillsPromise = null;
      }
    });

  cachedManagedSkillsPromise = request;
  return request;
}

export function __resetSkillsContextCacheForTests() {
  cachedManagedSkills = null;
  cachedManagedSkillsPromise = null;
}

interface SkillsContextType {
  skills: SkillMetadata[];
  isLoading: boolean;
  error: string | null;
  refreshSkills: () => Promise<void>;
}

const SkillsContext = createContext<SkillsContextType | undefined>(undefined);

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<SkillMetadata[]>(
    cachedManagedSkills ?? [],
  );
  // Start as true so toast never fires before the first fetch completes
  const [isLoading, setIsLoading] = useState(cachedManagedSkills === null);
  const [error, setError] = useState<string | null>(null);
  const { isLoading: settingsLoading } = useSettings();

  const fetchSkills = useCallback(async (forceRefresh = false) => {
    setIsLoading(true);
    setError(null);

    try {
      const result = await loadManagedSkills(forceRefresh);
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
  }, []);

  // Initial fetch - wait for settings to load, then scan.
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
        refreshSkills: () => fetchSkills(true),
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
