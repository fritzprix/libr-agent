import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { DialogProps } from '@radix-ui/react-dialog';
import { useCallback, useEffect, useState } from 'react';
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  InputWithLabel,
  TextareaWithLabel,
} from '../../components/ui';
import LocalServicesEditor from './LocalServicesEditor';
import MCPConfigEditor from './MCPConfigEditor';

export default function AssistantEditor() {
  const { draft, update } = useEditor<Assistant>();
  const [mcpConfigText, setMcpConfigText] = useState<string>(
    JSON.stringify(draft.mcpConfig, null, 2),
  );

  const handleMCPConfigUpdate = useCallback(
    (jsonString: string) => {
      setMcpConfigText(jsonString);
      update((draft) => {
        try {
          draft.mcpConfig = JSON.parse(jsonString);
        } catch {
          // do nothing
        }
      });
    },
    [setMcpConfigText, update],
  );

  return (
    <div className="flex-1 flex flex-col h-full">
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-4">
          <InputWithLabel
            label="어시스턴트 이름 *"
            value={draft?.name || ''}
            onChange={(e) =>
              update((draft) => {
                draft.name = e.target.value;
              })
            }
            placeholder="어시스턴트 이름을 입력하세요..."
          />

          <TextareaWithLabel
            label="시스템 프롬프트 *"
            value={draft?.systemPrompt || ''}
            onChange={(e) =>
              update((draft) => {
                draft.systemPrompt = e.target.value;
              })
            }
            placeholder="AI가 수행할 역할과 행동 방식을 설명하세요..."
            className="h-32"
          />
          <LocalServicesEditor />
          <MCPConfigEditor
            mcpConfigText={mcpConfigText}
            onChange={handleMCPConfigUpdate}
          />
        </div>
      </div>
    </div>
  );
}

function AssistantDialog(props: DialogProps) {
  const { draft, commit } = useEditor<Assistant>();
  const [open, setOpen] = useState(props.open ?? false);

  // Keep dialog open state in sync with props
  useEffect(() => {
    if (typeof props.open === 'boolean') setOpen(props.open);
  }, [props.open]);

  const handleSave = () => {
    commit();
    setOpen(false);
    if (props.onOpenChange) props.onOpenChange(false);
  };
  const handleCancel = () => {
    setOpen(false);
    if (props.onOpenChange) props.onOpenChange(false);
  };

  return (
    <Dialog {...props} open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-2xl h-fit p-0 flex flex-col">
        <DialogHeader>
          <DialogTitle className="p-2">
            {draft.id ? '어시스턴트 편집' : '새 어시스턴트 만들기'}
          </DialogTitle>
          <DialogDescription>
            <AssistantEditor />
          </DialogDescription>
        </DialogHeader>
        <div className="flex justify-end gap-2 p-4 border-t">
          <Button variant="outline" onClick={handleCancel}>
            취소
          </Button>
          <Button variant="default" onClick={handleSave}>
            저장
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

AssistantEditor.Dialog = AssistantDialog;
