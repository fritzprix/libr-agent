import { useCallback, useEffect, useState } from 'react';
import {
  ButtonLegacy as Button,
  InputWithLabel as Input,
  Modal,
} from '../../components/ui';
import { useAssistantContext } from '@/context/AssistantContext';
import { Assistant, Group } from '@/models/chat';

interface GroupCreationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (group: Partial<Group>) => void;
  editingGroup: Partial<Group> | null;
}

export default function GroupCreationModal({
  isOpen,
  onClose,
  onSave,
  editingGroup,
}: GroupCreationModalProps) {
  const { assistants } = useAssistantContext();
  const [groupName, setGroupName] = useState('');
  const [groupDescription, setGroupDescription] = useState('');
  const [selectedAssistants, setSelectedAssistants] = useState<Assistant[]>([]);

  useEffect(() => {
    if (editingGroup) {
      setGroupName(editingGroup.name || '');
      setGroupDescription(editingGroup.description || '');
      setSelectedAssistants(editingGroup.assistants || []);
    } else {
      setGroupName('');
      setGroupDescription('');
      setSelectedAssistants([]);
    }
  }, [editingGroup]);

  const handleToggleAssistant = (assistant: Assistant) => {
    setSelectedAssistants((prev) =>
      prev.some((a) => a.id === assistant.id)
        ? prev.filter((a) => a.id !== assistant.id)
        : [...prev, assistant],
    );
  };

  const handleSave = useCallback(() => {
    if (!groupName.trim() || selectedAssistants.length === 0) {
      alert('Please provide a group name and select at least one assistant.');
      return;
    }

    onSave({
      id: editingGroup?.id,
      name: groupName,
      description: groupDescription,
      assistants: selectedAssistants,
    });
    onClose();
  }, [
    groupName,
    groupDescription,
    selectedAssistants,
    onSave,
    onClose,
    editingGroup,
  ]);

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Create New Group" size="lg">
      <div className="p-4 flex flex-col h-full">
        <Input
          label="Group Name"
          placeholder="e.g., Marketing Team AI"
          value={groupName}
          onChange={(e) => setGroupName(e.target.value)}
          className="mb-4"
        />
        <Input
          label="Group Description"
          placeholder="e.g., A group for marketing related queries and content generation"
          value={groupDescription}
          onChange={(e) => setGroupDescription(e.target.value)}
          className="mb-4"
        />

        <h3 className="text-lg font-semibold mb-3">Select Assistants</h3>
        <div className="flex-1 overflow-y-auto border border-gray-700 rounded-md p-3 space-y-2 terminal-scrollbar">
          {assistants.length === 0 ? (
            <p className="text-gray-500">
              No assistants available. Please add some in settings.
            </p>
          ) : (
            assistants.map((assistant) => (
              <div
                key={assistant.id}
                className={`flex items-center justify-between p-2 rounded-md cursor-pointer ${selectedAssistants.some((a) => a.id === assistant.id) ? 'bg-primary/20 border border-primary' : 'hover:bg-gray-700'}`}
                onClick={() => handleToggleAssistant(assistant)}
              >
                <div>
                  <p className="font-medium text-primary">{assistant.name}</p>
                  <p className="text-xs text-gray-400 line-clamp-1">
                    {assistant.systemPrompt}
                  </p>
                </div>
                {selectedAssistants.some((a) => a.id === assistant.id) && (
                  <span className="text-primary">✓</span>
                )}
              </div>
            ))
          )}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleSave}
            disabled={selectedAssistants.length === 0 || !groupName.trim()}
          >
            {editingGroup ? 'Save Group' : 'Create Group'}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
