import { useCallback } from 'react';
import { useAssistantContext } from '../context/AssistantContext';
import { useSessionContext } from '../context/SessionContext';
import { Assistant } from '../types/chat';
import { Button } from './ui';
import AssistantDetailList from './assistant/AssistantDetailList';

interface StartSingleChatViewProps {
  setShowAssistantManager: (show: boolean) => void;
  showAssistantManager: boolean;
}

export default function StartSingleChatView({
  setShowAssistantManager,
  showAssistantManager,
}: StartSingleChatViewProps) {
  const { assistants, setCurrentAssistant } = useAssistantContext();
  const { start } = useSessionContext();

  const handleAssistantSelect = useCallback(
    (assistant: Assistant) => {
      setCurrentAssistant(assistant);
      start([assistant]);
    },
    [start, setCurrentAssistant],
  );

  return (
    <div className="h-full w-full flex flex-col items-center justify-center font-mono p-4">
      <h2 className="text-2xl font-bold mb-6">
        Select an Assistant to Start a Chat
      </h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 w-full max-w-4xl">
        {assistants.map((assistant) => (
          <div
            key={assistant.id}
            className=" border rounded-lg p-4 cursor-pointer   transition-colors"
            onClick={() => handleAssistantSelect(assistant)}
          >
            <h3 className="text-lg font-semibold">{assistant.name}</h3>
            <p className="text-sm mt-2 line-clamp-3">
              {assistant.systemPrompt}
            </p>
          </div>
        ))}
      </div>
      <Button onClick={() => setShowAssistantManager(true)} className="mt-8">
        Manage Assistants
      </Button>
      {showAssistantManager && <AssistantDetailList />}
    </div>
  );
}
