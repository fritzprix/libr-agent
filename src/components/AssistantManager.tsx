"use client";

import { useCallback, useState } from "react";
import {
  DEFAULT_MCP_CONFIG,
  useAssistantContext,
} from "../context/AssistantContext";
import { useLocalTools } from "../context/LocalToolContext";
import { useMCPServer } from "../hooks/use-mcp-server";
import { getLogger } from "../lib/logger";
import { Assistant } from "../types/chat";
import {
  BadgeLegacy as Badge,
  ButtonLegacy as Button,
  InputWithLabel as Input,
  Modal,
  StatusIndicator,
  TextareaWithLabel as Textarea,
} from "./ui";

const logger = getLogger("AssistantManager");

interface AssistantManagerProps {
  onClose: () => void;
}

export default function AssistantManager({ onClose }: AssistantManagerProps) {
  const {
    currentAssistant,
    assistants,
    saveAssistant,
    deleteAssistant,
    setCurrentAssistant,
    getNewAssistantTemplate,
  } = useAssistantContext();
  const { status, isConnecting: isCheckingStatus } = useMCPServer();
  const { getAvailableServices, getToolsByService } = useLocalTools();

  const [editingAssistant, setEditingAssistant] =
    useState<Partial<Assistant> | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [mcpConfigText, setMcpConfigText] = useState("");
  const [isDeleting, setIsDeleting] = useState<string | null>(null);

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

    logger.info("saving assistant: ", { editingAssistant });

    const savedAssistant = await saveAssistant(editingAssistant, mcpConfigText);
    if (savedAssistant) {
      setEditingAssistant(null);
      setIsCreating(false);
      setMcpConfigText("");
      logger.debug(`Assistant "${savedAssistant.name}" saved successfully`);
    }
  };

  const handleSelectAssistant = useCallback(
    (assistant: Assistant) => {
      setCurrentAssistant(assistant);
    },
    [setCurrentAssistant],
  );

  const handleDeleteAssistant = async (assistant: Assistant) => {
    // Check if it's a default role
    if (assistant.isDefault) {
      alert("기본 어시스턴트는 삭제할 수 없습니다.");
      return;
    }

    try {
      setIsDeleting(assistant.id);
      logger.info(
        `Attempting to delete assistant: ${assistant.name} (${assistant.id})`,
      );

      // If we're editing this assistant, cancel editing first
      if (editingAssistant?.id === assistant.id) {
        logger.info("cancel!!!");
        handleCancel();
      }

      // If this is the current assistant, we might want to switch to a default assistant
      if (currentAssistant?.id === assistant.id) {
        const defaultAssistant = assistants.find(
          (a) => a.isDefault && a.id !== assistant.id,
        );
        if (defaultAssistant) {
          setCurrentAssistant(defaultAssistant);
        }
      }

      // Delete the assistant (confirmation will be handled by the context)
      await deleteAssistant(assistant.id);

      logger.info(
        `Successfully deleted assistant: ${assistant.name} (${assistant.id})`,
      );
    } catch (error) {
      logger.error(
        `Failed to delete assistant: ${assistant.name} (${assistant.id})`,
        { error },
      );
      alert(
        `어시스턴트 삭제에 실패했습니다: ${error instanceof Error ? error.message : "Unknown error"}`,
      );
    } finally {
      setIsDeleting(null);
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
    >
      {/* Main container with fixed height */}
      <div className="flex h-full overflow-hidden flex-col md:flex-row">
        {/* Assistant List - Fixed width with internal scrolling */}
        <div className="w-full md:w-1/3 border-r border-gray-700 flex flex-col h-full">
          {/* Button - Fixed at top */}
          <div className="p-4 border-b border-gray-700 flex-shrink-0">
            <Button
              variant="primary"
              className="w-full"
              onClick={handleCreateNew}
            >
              + 새 어시스턴트 만들기
            </Button>
          </div>

          {/* Scrollable assistants list */}
          <div className="flex-1 overflow-y-auto p-4">
            <div className="space-y-2">
              {assistants.map((assistant) => (
                <div
                  key={assistant.id}
                  className={`border rounded p-3 cursor-pointer transition-colors ${
                    currentAssistant?.id === assistant.id
                      ? "border-green-400 bg-green-900/20"
                      : "border-gray-700 hover:border-gray-600"
                  }`}
                >
                  <div className="flex justify-between items-start mb-2">
                    <h3 className="text-green-400 font-medium">
                      {assistant.name}
                    </h3>
                    <div className="flex gap-1 flex-wrap">
                      {assistant.isDefault && (
                        <Badge variant="warning">DEFAULT</Badge>
                      )}
                      {currentAssistant?.id === assistant.id && (
                        <Badge variant="active">ACTIVE</Badge>
                      )}
                    </div>
                  </div>
                  <p className="text-gray-400 text-sm mb-3 line-clamp-2">
                    {assistant.systemPrompt}
                  </p>
                  <div className="text-xs text-gray-500 mb-2">
                    MCP 서버:{" "}
                    {assistant.mcpConfig?.mcpServers
                      ? Object.keys(assistant.mcpConfig.mcpServers).length
                      : 0}
                    개, 로컬 서비스: {assistant.localServices?.length || 0}개
                  </div>

                  {currentAssistant?.id === assistant.id && (
                    <div className="flex flex-wrap gap-1 mb-2">
                      {Object.keys(assistant.mcpConfig?.mcpServers || {}).map(
                        (serverName) => (
                          <div
                            key={serverName}
                            className="flex items-center gap-1 text-xs px-1 py-0.5 rounded bg-gray-800"
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
                            <span className="text-gray-300">{serverName}</span>
                          </div>
                        ),
                      )}
                    </div>
                  )}

                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => handleSelectAssistant(assistant)}
                      disabled={currentAssistant?.id === assistant.id}
                    >
                      선택
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => handleEditAssistant(assistant)}
                    >
                      편집
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={isCheckingStatus}
                    >
                      {isCheckingStatus && currentAssistant?.id === assistant.id
                        ? "확인중..."
                        : "상태확인"}
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => handleDeleteAssistant(assistant)}
                      // disabled={assistant.isDefault || isDeleting === assistant.id}
                      title={
                        assistant.isDefault
                          ? "기본 어시스턴트는 삭제할 수 없습니다."
                          : "어시스턴트 삭제"
                      }
                    >
                      {isDeleting === assistant.id ? "삭제중..." : "삭제"}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Role Editor */}
        {(editingAssistant || isCreating) && (
          <div className="flex-1 flex flex-col h-full">
            <div className="p-4 border-b border-gray-700 flex-shrink-0">
              <h3 className="text-green-400 font-bold text-lg">
                {isCreating ? "새 어시스턴트 만들기" : "어시스턴트 편집"}
              </h3>
            </div>

            <div className="flex-1 overflow-y-auto p-4">
              <div className="space-y-4">
                <Input
                  label="어시스턴트 이름 *"
                  value={editingAssistant?.name || ""}
                  onChange={(e) =>
                    setEditingAssistant((prev) => ({
                      ...prev,
                      name: e.target.value,
                    }))
                  }
                  placeholder="어시스턴트 이름을 입력하세요..."
                />

                <Textarea
                  label="시스템 프롬프트 *"
                  value={editingAssistant?.systemPrompt || ""}
                  onChange={(e) =>
                    setEditingAssistant((prev) => ({
                      ...prev,
                      systemPrompt: e.target.value,
                    }))
                  }
                  placeholder="AI가 수행할 역할과 행동 방식을 설명하세요..."
                  className="h-32"
                />
              </div>

              <div>
                <label className="text-gray-400 font-medium">
                  로컬 서비스 활성화
                </label>
                <div className="space-y-2 mt-2 p-2 border border-gray-700 rounded-md">
                  {getAvailableServices().map((serviceName) => (
                    <div key={serviceName}>
                      <h4 className="text-sm font-semibold text-gray-300 mb-1">
                        {serviceName}
                      </h4>
                      <div className="space-y-1 pl-2">
                        {getToolsByService(serviceName).map((tool) => (
                          <div key={tool.name} className="flex items-center">
                            <input
                              type="checkbox"
                              id={`tool-${tool.name}`}
                              checked={
                                editingAssistant?.localServices?.includes(
                                  serviceName,
                                ) || false
                              }
                              onChange={(e) => {
                                const newLocalServices = e.target.checked
                                  ? [
                                      ...(editingAssistant?.localServices ||
                                        []),
                                      serviceName,
                                    ]
                                  : editingAssistant?.localServices?.filter(
                                      (s: string) => s !== serviceName,
                                    ) || [];
                                setEditingAssistant((prev) => ({
                                  ...prev,
                                  localServices: newLocalServices,
                                }));
                              }}
                              className="mr-2"
                            />
                            <label
                              htmlFor={`tool-${tool.name}`}
                              className="text-sm text-gray-400"
                            >
                              {tool.name}
                            </label>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>

                <div>
                  <div className="flex justify-between items-center mb-2">
                    <label className="text-gray-400 font-medium">
                      MCP 설정 (JSON)
                      <span className="text-xs text-gray-500 ml-2">
                        - 연결할 MCP 서버들을 설정합니다
                      </span>
                    </label>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={isCheckingStatus}
                      >
                        {isCheckingStatus ? "확인중..." : "서버 상태 확인"}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={handleFormatJson}
                      >
                        Format JSON
                      </Button>
                    </div>
                  </div>
                  <Textarea
                    value={mcpConfigText}
                    onChange={(e) => setMcpConfigText(e.target.value)}
                    className="h-48 font-mono text-sm"
                    placeholder={JSON.stringify(DEFAULT_MCP_CONFIG)}
                  />
                  <div className="text-xs text-gray-500 mt-1">
                    * mcpServers 형식만 사용됩니다. 빈 객체로 두면 MCP 서버를
                    사용하지 않습니다.
                  </div>

                  {editingAssistant && Object.keys(status).length > 0 && (
                    <div className="mt-3 p-2 bg-gray-900 rounded border border-gray-700">
                      <div className="text-xs text-gray-400 mb-2">
                        에디터 서버 상태:
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {Object.entries(status).map(
                          ([serverName, isConnected]) => (
                            <div
                              key={serverName}
                              className="flex items-center gap-1 text-xs px-2 py-1 rounded bg-gray-800"
                            >
                              <StatusIndicator
                                status={
                                  isConnected ? "connected" : "disconnected"
                                }
                              />
                              <span className="text-gray-300">
                                {serverName}
                              </span>
                              <Badge
                                variant={isConnected ? "success" : "error"}
                              >
                                {isConnected ? "OK" : "NOK"}
                              </Badge>
                            </div>
                          ),
                        )}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>

            <div className="flex gap-3 p-4 border-t border-gray-700 flex-shrink-0">
              <Button variant="primary" onClick={handleSaveAssistant}>
                저장
              </Button>
              <Button variant="secondary" onClick={handleCancel}>
                취소
              </Button>
            </div>
          </div>
        )}

        {!editingAssistant && !isCreating && (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center text-gray-500">
              <div className="text-4xl mb-4">⚙️</div>
              <p>어시스턴트를 선택하여 편집하거나</p>
              <p>새 어시스턴트를 만들어보세요</p>
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}
