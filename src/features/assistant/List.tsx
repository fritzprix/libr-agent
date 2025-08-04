import {
  getNewAssistantTemplate,
  useAssistantContext,
} from '@/context/AssistantContext';
import { EditorProvider } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { useCallback, useState } from 'react';
import { Button } from '../../components/ui';
import AssistantEditor from './AssistantEditor';
import AssistantCard from './Card';

export default function AssistantList() {
  const { assistants, saveAssistant } = useAssistantContext();
  const [createNew, setCreateNew] = useState<boolean>(false);
  const handleCreateComplete = useCallback(
    (assistant: Assistant) => {
      saveAssistant(assistant);
    },
    [saveAssistant],
  );

  return (
    <div className="w-full border-r border-muted flex flex-col h-full">
      {/* Button - Fixed at top */}
      <div className="p-4 border-b border-muted flex-shrink-0">
        <Button
          variant="default"
          className="w-full"
          onClick={() => setCreateNew(true)}
        >
          + 새 어시스턴트 만들기
        </Button>
      </div>

      {/* Scrollable assistants list */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-2">
          {assistants.map((assistant) => (
            <AssistantCard key={assistant.id} assistant={assistant} />
          ))}
        </div>
      </div>
      <EditorProvider
        initialValue={getNewAssistantTemplate()}
        onFinalize={handleCreateComplete}
      >
        <AssistantEditor.Dialog open={createNew} onOpenChange={setCreateNew} />
      </EditorProvider>
    </div>
  );
}
