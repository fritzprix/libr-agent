import { useAssistantContext } from "@/context/AssistantContext";
import { Assistant } from "@/types/chat";
import { Button } from "../ui";
import AssistantCard from "./Card";

interface AssistantListProps {
  onCreateNew: () => void;
  onEditAssistant: (assistant: Assistant) => void;
}

export default function AssistantList({
  onCreateNew,
  onEditAssistant,
}: AssistantListProps) {
  const { assistants } = useAssistantContext();

  return (
    <div className="w-full border-r border-muted flex flex-col h-full">
      {/* Button - Fixed at top */}
      <div className="p-4 border-b border-muted flex-shrink-0">
        <Button variant="default" className="w-full" onClick={onCreateNew}>
          + 새 어시스턴트 만들기
        </Button>
      </div>

      {/* Scrollable assistants list */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-2">
          {assistants.map((assistant) => (
            <AssistantCard
              key={assistant.id}
              assistant={assistant}
              onEdit={onEditAssistant}
            />
          ))}
        </div>
      </div>
    </div>
  );
}