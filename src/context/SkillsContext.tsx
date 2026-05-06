import { safeInvoke } from '@/lib/backend/core';
import React, {
  createContext,
  useContext,
  useEffect,
  useCallback,
  useRef,
  useState,
} from 'react';
import { getLogger } from '@/lib/logger';
import { useSettings } from '@/hooks/use-settings';

import { SkillMetadata } from '@/types/skills';

const logger = getLogger('SkillsContext');

let cachedManagedSkills: SkillMetadata[] | null = null;
let cachedManagedSkillsPromise: Promise<SkillMetadata[]> | null = null;
let managedSkillsCacheGeneration = 0;

function invalidateManagedSkillsCache() {
  managedSkillsCacheGeneration += 1;
  cachedManagedSkills = null;
  cachedManagedSkillsPromise = null;
}

async function loadManagedSkills(
  forceRefresh = false,
): Promise<SkillMetadata[]> {
  if (forceRefresh) {
    invalidateManagedSkillsCache();
  }

  if (!forceRefresh && cachedManagedSkills !== null) {
    return cachedManagedSkills;
  }

  if (!forceRefresh && cachedManagedSkillsPromise) {
    return cachedManagedSkillsPromise;
  }

  const requestGeneration = managedSkillsCacheGeneration;
  const request = safeInvoke<{ effectiveSkills: SkillMetadata[] }>(
    'get_managed_skills_overview',
  )
    .then((overview) => {
      const result = overview.effectiveSkills ?? [];
      if (
        requestGeneration === managedSkillsCacheGeneration &&
        cachedManagedSkillsPromise === request
      ) {
        cachedManagedSkills = result;
      }
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
  invalidateManagedSkillsCache();
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
  const fetchRequestVersionRef = useRef(0);

  const fetchSkills = useCallback(async (forceRefresh = false) => {
    const requestVersion = ++fetchRequestVersionRef.current;
    setIsLoading(true);
    setError(null);

    try {
      const result = await loadManagedSkills(forceRefresh);
      if (requestVersion !== fetchRequestVersionRef.current) {
        return;
      }

      logger.info('Discovered skills:', {
        count: result.length,
        skills: result,
      });
      setSkills(result);
    } catch (err) {
      if (requestVersion !== fetchRequestVersionRef.current) {
        return;
      }

      const errMsg = err instanceof Error ? err.message : String(err);
      logger.warn('Failed to scan skills directory', err);
      setError(`Failed to scan skills: ${errMsg}`);
      setSkills([]);
    } finally {
      if (requestVersion === fetchRequestVersionRef.current) {
        setIsLoading(false);
      }
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
