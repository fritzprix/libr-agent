'use client';

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui';
import { Assistant } from '@/models/chat';
import { useState } from 'react';
import {
  getNewAssistantTemplate,
  useAssistantContext,
} from '../../context/AssistantContext';
import AssistantEditor from './AssistantEditor';
import EmptyState from './EmptyState';
import AssistantList from './List';

export default function AssistantEditorView() {
  const { upsert } = useAssistantContext();

  const [editingAssistant, setEditingAssistant] =
    useState<Partial<Assistant> | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [mcpConfigText, setMcpConfigText] = useState('');

  const handleCreateNew = () => {
    setIsCreating(true);
    const { assistant, mcpConfigText } = getNewAssistantTemplate();
    setEditingAssistant(assistant);
    setMcpConfigText(mcpConfigText);
  };

  const handleEditAssistant = (assistant: Assistant) => {
    setEditingAssistant(assistant);
    setIsCreating(false);
    setMcpConfigText(JSON.stringify(assistant.mcpConfig, null, 2));
  };

  const handleSaveAssistant = async () => {
    if (!editingAssistant) return;

    const savedAssistant = await upsert(editingAssistant, mcpConfigText);
    if (savedAssistant) {
      handleCancel();
    }
  };

  const handleCancel = () => {
    setEditingAssistant(null);
    setIsCreating(false);
    setMcpConfigText('');
  };

  const handleFormatJson = () => {
    try {
      const parsed = JSON.parse(mcpConfigText);
      setMcpConfigText(JSON.stringify(parsed, null, 2));
    } catch {
      alert('유효하지 않은 JSON 형식입니다. JSON을 확인해주세요.');
    }
  };

  return (
    <div className="flex h-full overflow-auto flex-col md:flex-row">
      <AssistantList
        onCreateNew={handleCreateNew}
        onEditAssistant={handleEditAssistant}
      />

      <Dialog open={!!editingAssistant} onOpenChange={handleCancel}>
        <DialogContent className="max-w-2xl h-fit p-0 flex flex-col">
          <DialogHeader>
            <DialogTitle className="p-2">
              {isCreating ? '새 어시스턴트 만들기' : '어시스턴트 편집'}
            </DialogTitle>
            <DialogDescription>
              {(editingAssistant || isCreating) && (
                <AssistantEditor
                  editingAssistant={editingAssistant}
                  isCreating={isCreating}
                  mcpConfigText={mcpConfigText}
                  onSave={handleSaveAssistant}
                  onCancel={handleCancel}
                  onAssistantChange={setEditingAssistant}
                  onMcpConfigChange={setMcpConfigText}
                  onFormatJson={handleFormatJson}
                />
              )}
            </DialogDescription>
          </DialogHeader>
        </DialogContent>
      </Dialog>

      {!editingAssistant && !isCreating && <EmptyState />}
    </div>
  );
}
