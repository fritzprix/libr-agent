import LocalServicesEditor from './LocalServicesEditor';
import MCPConfigEditor from './MCPConfigEditor';
import { Button, InputWithLabel, TextareaWithLabel } from '../../components/ui';
import { Assistant } from '@/models/chat';

interface AssistantEditorProps {
  editingAssistant: Partial<Assistant> | null;
  isCreating: boolean;
  mcpConfigText: string;
  onSave: () => void;
  onCancel: () => void;
  onAssistantChange: (assistant: Partial<Assistant>) => void;
  onMcpConfigChange: (text: string) => void;
  onFormatJson: () => void;
}

export default function AssistantEditor({
  editingAssistant,
  isCreating,
  mcpConfigText,
  onSave,
  onCancel,
  onAssistantChange,
  onMcpConfigChange,
  onFormatJson,
}: AssistantEditorProps) {
  const handleLocalServicesChange = (localServices: string[]) => {
    onAssistantChange({ ...editingAssistant, localServices });
  };

  return (
    <div className="flex-1 flex flex-col h-full">
      <div className="p-4 border-b border-muted flex-shrink-0">
        <h3 className="text-primary font-bold text-lg">
          {isCreating ? '새 어시스턴트 만들기' : '어시스턴트 편집'}
        </h3>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-4">
          <InputWithLabel
            label="어시스턴트 이름 *"
            value={editingAssistant?.name || ''}
            onChange={(e) =>
              onAssistantChange({
                ...editingAssistant,
                name: e.target.value,
              })
            }
            placeholder="어시스턴트 이름을 입력하세요..."
          />

          <TextareaWithLabel
            label="시스템 프롬프트 *"
            value={editingAssistant?.systemPrompt || ''}
            onChange={(e) =>
              onAssistantChange({
                ...editingAssistant,
                systemPrompt: e.target.value,
              })
            }
            placeholder="AI가 수행할 역할과 행동 방식을 설명하세요..."
            className="h-32"
          />

          <LocalServicesEditor
            localServices={editingAssistant?.localServices}
            onChange={handleLocalServicesChange}
          />

          <MCPConfigEditor
            mcpConfigText={mcpConfigText}
            onChange={onMcpConfigChange}
            onFormatJson={onFormatJson}
          />
        </div>
      </div>

      <div className="flex gap-3 p-4 border-t border-muted flex-shrink-0">
        <Button variant="default" onClick={onSave}>
          저장
        </Button>
        <Button variant="secondary" onClick={onCancel}>
          취소
        </Button>
      </div>
    </div>
  );
}
