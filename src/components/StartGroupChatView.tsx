import { useCallback } from 'react';
import { useAssistantGroupContext } from '../context/AssistantGroupContext';
import { useSessionContext } from '../context/SessionContext';
import { Group } from '../types/chat';
import { Button } from './ui';
import AssistantGroupDetailList from './assistant/AssistantGroupDetailList';

interface StartGroupChatViewProps {
  setShowAssistantGroupManager: (show: boolean) => void;
  showAssistantGroupManager: boolean;
}

export default function StartGroupChatView({
  setShowAssistantGroupManager,
  showAssistantGroupManager,
}: StartGroupChatViewProps) {
  const { groups, setCurrentGroup } = useAssistantGroupContext();
  const { start } = useSessionContext();

  const handleGroupSelect = useCallback(
    (group: Group) => {
      setCurrentGroup(group);
      start(group.assistants, group.name, group.description); // Pass group details to start session
    },
    [start, setCurrentGroup],
  );

  return (
    <div className="h-full w-full flex flex-col items-center justify-center font-mono p-4">
      <h2 className="text-2xl font-bold mb-6">
        Select an Assistant Group to Start a Chat
      </h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 w-full max-w-4xl">
        {groups.map((group) => (
          <div
            key={group.id}
            className=" border rounded-lg p-4 cursor-pointer   transition-colors"
            onClick={() => handleGroupSelect(group)}
          >
            <h3 className="text-lg font-semibold">{group.name}</h3>
            <p className="text-sm mt-2 line-clamp-3">{group.description}</p>
          </div>
        ))}
      </div>
      <Button
        onClick={() => setShowAssistantGroupManager(true)}
        className="mt-8"
      >
        Manage Assistant Groups
      </Button>
      {showAssistantGroupManager && <AssistantGroupDetailList />}
    </div>
  );
}
