import { useAssistantContext } from "@/context/AssistantContext";
import { useMCPServer } from "@/hooks/use-mcp-server";
import { useState } from "react";
import { Badge, Button, StatusIndicator } from "../ui";
import { Assistant } from "@/types/chat";

interface AssistantCardProps {
  assistant: Assistant;
  onEdit: (assistant: Assistant) => void;
}

export default function AssistantCard({ assistant, onEdit }: AssistantCardProps) {
  const { currentAssistant, setCurrentAssistant, delete: deleteAssistant } = useAssistantContext();
  const { status, isConnecting: isCheckingStatus } = useMCPServer();
  const [isDeleting, setIsDeleting] = useState(false);
  
  const isActive = currentAssistant?.id === assistant.id;

  const handleDelete = async () => {
    if (assistant.isDefault) {
      alert("기본 어시스턴트는 삭제할 수 없습니다.");
      return;
    }

    try {
      setIsDeleting(true);
      await deleteAssistant(assistant.id);
    } finally {
      setIsDeleting(false);
    }
  };
  return (
    <div
      className={`border rounded p-3 cursor-pointer transition-colors ${
        isActive
          ? "border-primary bg-primary/20"
          : "border-muted hover:border-accent"
      }`}
    >
      <div className="flex justify-between items-start mb-2">
        <h3 className="text-primary font-medium">{assistant.name}</h3>
        <div className="flex gap-1 flex-wrap">
          {assistant.isDefault && <Badge variant="destructive">DEFAULT</Badge>}
          {isActive && <Badge variant="default">ACTIVE</Badge>}
        </div>
      </div>

      <p className="text-muted-foreground text-sm mb-3 line-clamp-2">
        {assistant.systemPrompt}
      </p>

      <div className="text-xs text-muted-foreground mb-2">
        MCP 서버:{" "}
        {assistant.mcpConfig?.mcpServers
          ? Object.keys(assistant.mcpConfig.mcpServers).length
          : 0}
        개, 로컬 서비스: {assistant.localServices?.length || 0}개
      </div>

      {isActive && (
        <div className="flex flex-wrap gap-1 mb-2">
          {Object.keys(assistant.mcpConfig?.mcpServers || {}).map(
            (serverName) => (
              <div
                key={serverName}
                className="flex items-center gap-1 text-xs px-1 py-0.5 rounded bg-muted"
              >
                <StatusIndicator
                  status={
                    status[serverName] === true
                      ? "connected"
                      : status[serverName] === false
                      ? "disconnected"
                      : "unknown"
                  }
                  size="sm"
                />
                <span className="text-foreground">{serverName}</span>
              </div>
            )
          )}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="default"
          onClick={() => setCurrentAssistant(assistant)}
          disabled={isActive}
        >
          선택
        </Button>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => onEdit(assistant)}
        >
          편집
        </Button>
        <Button size="sm" variant="ghost" disabled={isCheckingStatus}>
          {isCheckingStatus && isActive ? "확인중..." : "상태확인"}
        </Button>
        <Button
          size="sm"
          variant="destructive"
          onClick={handleDelete}
          title={
            assistant.isDefault
              ? "기본 어시스턴트는 삭제할 수 없습니다."
              : "어시스턴트 삭제"
          }
        >
          {isDeleting ? "삭제중..." : "삭제"}
        </Button>
      </div>
    </div>
  );
}