"use client";

import { useState } from "react";
import AssistantEditor from "./AssistantEditor";
import EmptyState from "./EmptyState";
import { getNewAssistantTemplate, useAssistantContext } from "@/context/AssistantContext";
import { Assistant } from "@/types/chat";
import { Modal } from "../ui";
import AssistantList from "./List";

interface AssistantManagerProps {
  onClose: () => void;
}

export default function AssistantManager({ onClose }: AssistantManagerProps) {
  const { upsert } = useAssistantContext();

  const [editingAssistant, setEditingAssistant] =
    useState<Partial<Assistant> | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [mcpConfigText, setMcpConfigText] = useState("");

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
    setMcpConfigText("");
  };

  const handleFormatJson = () => {
    try {
      const parsed = JSON.parse(mcpConfigText);
      setMcpConfigText(JSON.stringify(parsed, null, 2));
    } catch {
      alert("유효하지 않은 JSON 형식입니다. JSON을 확인해주세요.");
    }
  };

  return (
    <Modal
      isOpen={true}
      onClose={onClose}
      title="Assistant Management"
      size="xl"
      className="w-fit"
    >
      <div className="flex h-full overflow-auto flex-col md:flex-row">
        <AssistantList
          onCreateNew={handleCreateNew}
          onEditAssistant={handleEditAssistant}
        />

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

        {!editingAssistant && !isCreating && <EmptyState />}
      </div>
    </Modal>
  );
}