import { createId } from '@paralleldrive/cuid2';
import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from 'react';
import { dbService } from '../lib/db';
import { getLogger } from '../lib/logger';
import { Group } from '../models/chat';

const logger = getLogger('AssistantGroupContext');

interface AssistantGroupContextType {
  groups: Group[];
  currentGroup: Group | null;
  setCurrentGroup: (group: Group | null) => void;
  upsert: (group: Partial<Group>) => Promise<Group | undefined>;
  delete: (id: string) => Promise<void>;
  getGroupById: (id: string) => Group | undefined;
  getNewGroupTemplate: () => Group;
}

const AssistantGroupContext = createContext<
  AssistantGroupContextType | undefined
>(undefined);

export const AssistantGroupProvider: React.FC<{
  children: React.ReactNode;
}> = ({ children }) => {
  const [groups, setGroups] = useState<Group[]>([]);
  const [currentGroup, setCurrentGroup] = useState<Group | null>(null);

  const loadGroups = useCallback(async () => {
    try {
      const loadedGroups = await dbService.groups.getPage(1, -1); // Get all groups
      setGroups(loadedGroups.items);
      logger.info('Loaded assistant groups:', loadedGroups.items);
    } catch (error) {
      logger.error('Failed to load assistant groups:', { error });
    }
  }, []);

  useEffect(() => {
    loadGroups();
  }, [loadGroups]);

  const getNewGroupTemplate = useCallback((): Group => {
    return {
      id: createId(),
      name: 'New Group',
      description: '',
      assistants: [],
      createdAt: new Date(),
      updatedAt: new Date(),
    };
  }, []);

  const upsert = useCallback(
    async (group: Partial<Group>): Promise<Group | undefined> => {
      try {
        const groupToSave: Group = {
          ...getNewGroupTemplate(),
          ...group,
          updatedAt: new Date(),
        };

        if (!groupToSave.id) {
          groupToSave.id = createId();
          groupToSave.createdAt = new Date();
        }

        await dbService.groups.upsert(groupToSave);
        await loadGroups(); // Reload groups after upsert
        logger.info('Upserted group:', groupToSave);
        return groupToSave;
      } catch (error) {
        logger.error('Failed to upsert group:', { group, error });
        return undefined;
      }
    },
    [loadGroups, getNewGroupTemplate],
  );

  const deleteGroup = useCallback(
    async (id: string) => {
      try {
        await dbService.groups.delete(id);
        await loadGroups(); // Reload groups after deletion
        if (currentGroup?.id === id) {
          setCurrentGroup(null);
        }
        logger.info('Deleted group with ID:', id);
      } catch (error) {
        logger.error('Failed to delete group:', { id, error });
      }
    },
    [loadGroups, currentGroup],
  );

  const getGroupById = useCallback(
    (id: string) => {
      return groups.find((group) => group.id === id);
    },
    [groups],
  );

  const value = {
    groups,
    currentGroup,
    setCurrentGroup,
    upsert,
    delete: deleteGroup,
    getGroupById,
    getNewGroupTemplate,
  };

  return (
    <AssistantGroupContext.Provider value={value}>
      {children}
    </AssistantGroupContext.Provider>
  );
};

export const useAssistantGroupContext = () => {
  const context = useContext(AssistantGroupContext);
  if (context === undefined) {
    throw new Error(
      'useAssistantGroupContext must be used within an AssistantGroupProvider',
    );
  }
  return context;
};
