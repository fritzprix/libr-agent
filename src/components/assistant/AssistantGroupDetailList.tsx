import { useState } from 'react';
import { useAssistantGroupContext } from '@/context/AssistantGroupContext';
import { Group } from '@/types/chat';
import GroupCreationModal from '../GroupCreationModal';
import { Button } from '../ui';
import { Plus } from 'lucide-react';

interface AssistantGroupCardProps {
  group: Group;
  onEdit: (group: Group) => void;
}

function AssistantGroupCard({ group, onEdit }: AssistantGroupCardProps) {
  const { delete: deleteGroup } = useAssistantGroupContext();
  const [isDeleting, setIsDeleting] = useState(false);

  const handleDelete = async () => {
    try {
      setIsDeleting(true);
      await deleteGroup(group.id);
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div className="border rounded p-3 transition-colors hover:border-accent">
      <h3 className="text-primary font-medium mb-2">{group.name}</h3>
      <p className="text-muted-foreground text-sm mb-3 line-clamp-2">
        {group.description}
      </p>
      <div className="text-xs text-muted-foreground mb-2">
        어시스턴트: {group.assistants.length}개
      </div>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" variant="secondary" onClick={() => onEdit(group)}>
          편집
        </Button>
        <Button
          size="sm"
          variant="destructive"
          onClick={handleDelete}
          disabled={isDeleting}
        >
          {isDeleting ? '삭제중...' : '삭제'}
        </Button>
      </div>
    </div>
  );
}

export default function AssistantGroupDetailList() {
  const { groups, upsert, getNewGroupTemplate } = useAssistantGroupContext();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingGroup, setEditingGroup] = useState<Partial<Group> | null>(null);

  const handleCreateNew = () => {
    setEditingGroup(getNewGroupTemplate());
    setIsModalOpen(true);
  };

  const handleEditGroup = (group: Group) => {
    setEditingGroup(group);
    setIsModalOpen(true);
  };

  const handleSaveGroup = async (group: Partial<Group>) => {
    await upsert(group);
    setIsModalOpen(false);
    setEditingGroup(null);
  };

  const handleCloseModal = () => {
    setIsModalOpen(false);
    setEditingGroup(null);
  };

  return (
    <div className="flex h-full overflow-auto flex-col md:flex-row">
      <div className="flex-1 p-4">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-xl font-bold">Assistant Groups</h2>
          <Button onClick={handleCreateNew}>
            <Plus size={16} className="mr-2" />새 그룹 생성
          </Button>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {groups.length === 0 ? (
            <p className="text-muted-foreground">No assistant groups found.</p>
          ) : (
            groups.map((group) => (
              <AssistantGroupCard
                key={group.id}
                group={group}
                onEdit={handleEditGroup}
              />
            ))
          )}
        </div>
      </div>

      <GroupCreationModal
        isOpen={isModalOpen}
        onClose={handleCloseModal}
        onSave={handleSaveGroup}
        editingGroup={editingGroup}
      />
    </div>
  );
}
