import { createId } from '@paralleldrive/cuid2';
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useAssistantContext } from '../context/AssistantContext';
import { useLocalTools } from '../context/LocalToolContext';
import { useMCPServer } from '../hooks/use-mcp-server';
import { getLogger } from '../lib/logger';
import MessageBubble from './MessageBubble';
import { ToolCaller } from './orchestrators/ToolCaller';
import TerminalHeader from './TerminalHeader';
import ToolsModal from './ToolsModal';
import { Button, FileAttachment, InputWithLabel as Input } from './ui';
// import { useChatContext } from "../context/ChatContext";
import { Message } from '@/types/chat';
import { useSessionContext } from '../context/SessionContext';
import { useChatContext } from '../hooks/use-chat';
import AssistantDetailList from './assistant/AssistantDetailList';

const logger = getLogger('Chat');

interface ChatProps {
  children?: React.ReactNode;
  showAssistantManager: boolean;
  setShowAssistantManager: (show: boolean) => void;
}

export default function Chat({
  children,
  showAssistantManager,
  setShowAssistantManager,
}: ChatProps) {
  const { currentAssistant } = useAssistantContext();
  const [showToolsDetail, setShowToolsDetail] = useState(false);
  const [attachedFiles, setAttachedFiles] = useState<
    { name: string; content: string }[]
  >([]);
  const { availableTools: mcpTools } = useMCPServer();
  const { availableTools: localTools } = useLocalTools();
  const availableTools = useMemo(
    () => [...mcpTools, ...localTools],
    [mcpTools, localTools],
  );
  const [input, setInput] = useState('');
  const { current: currentSession } = useSessionContext();
  const { submit, isLoading, messages } = useChatContext();

  // Since ChatContainer ensures currentSession exists before rendering Chat,
  // we can safely assert it's not null here
  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleAgentInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setInput(e.target.value);
  };

  const handleFileAttachment = async (
    e: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const files = e.target.files;
    if (!files) return;

    const newAttachedFiles: { name: string; content: string }[] = [];

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      if (
        !file.type.startsWith('text/') &&
        !file.name.match(
          /\.(txt|md|json|js|ts|tsx|jsx|py|java|cpp|c|h|css|html|xml|yaml|yml|csv)$/i,
        )
      ) {
        alert(`File "${file.name}" is not a supported text file format.`);
        continue;
      }

      if (file.size > 1024 * 1024) {
        alert(`File "${file.name}" is too large. Maximum size is 1MB.`);
        continue;
      }

      try {
        const content = await file.text();
        newAttachedFiles.push({ name: file.name, content });
      } catch (error) {
        logger.error(`Error reading file ${file.name}:`, { error });
        alert(`Error reading file "${file.name}".`);
      }
    }

    setAttachedFiles((prev) => [...prev, ...newAttachedFiles]);
    e.target.value = '';
  };

  const removeAttachedFile = (index: number) => {
    setAttachedFiles((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    logger.info('submit!!', currentAssistant);
    e.preventDefault();
    if (!input.trim() && attachedFiles.length === 0) return;
    if (!currentAssistant) return;

    let messageContent = input.trim();
    if (attachedFiles.length > 0) {
      messageContent += '\n\n--- Attached Files ---\n';
      attachedFiles.forEach((file) => {
        messageContent += `\n[File: ${file.name}]\n${file.content}\n`;
      });
    }

    const userMessage: Message = {
      id: createId(),
      content: messageContent,
      role: 'user',
      sessionId: currentSession?.id || '', // Add sessionId
    };

    setInput('');
    setAttachedFiles([]);

    try {
      await submit([userMessage]);
    } catch (err) {
      logger.error('Error submitting message:', err);
    }
  };

  return (
    <div className="h-full w-full font-mono flex flex-col rounded-lg overflow-hidden shadow-2xl">
      <TerminalHeader>
        <Button
          className="text-xs px-2 py-1 rounded"
          onClick={() => setShowAssistantManager(true)}
        >
          [manage-assistants]
        </Button>
      </TerminalHeader>

      {/* Messages Area - Fills space between model picker and bottom UI */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="flex-1 p-4 overflow-y-auto flex flex-col gap-6 terminal-scrollbar">
          {messages.map((m) => (
            <MessageBubble
              key={m.id}
              message={m}
              currentAssistantName={currentSession?.assistants[0]?.name || ''}
            />
          ))}
          {isLoading && (
            <div className="flex justify-start">
              <div className="rounded px-3 py-2">
                <div className="text-xs mb-1">
                  Agent ({currentSession?.assistants[0]?.name})
                </div>
                <div className="text-sm">thinking...</div>
              </div>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Bottom UI Stack - Fixed at bottom */}
      <div className="flex-shrink-0">
        {/* Status bar */}
        <div className="px-4 py-2 border-t flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs">Assistant:</span>
            <span className="text-xs">
              {currentSession?.assistants[0]?.name || 'None'}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-xs">Tools:</span>
            <button
              onClick={() => setShowToolsDetail(true)}
              className="text-xs transition-colors flex items-center gap-1"
            >
              🔧 {availableTools.length} available
            </button>
          </div>
        </div>

        {/* Attached files section */}
        {attachedFiles.length > 0 && (
          <div className="px-4 py-2 border-t">
            <div className="text-xs mb-2">📎 Attached Files:</div>
            <div className="flex flex-wrap gap-2">
              {attachedFiles.map((file, index) => (
                <div
                  key={index}
                  className="flex items-center px-2 py-1 rounded border border-gray-700"
                >
                  <span className="text-xs truncate max-w-[150px]">
                    {file.name}
                  </span>
                  <button
                    type="button"
                    onClick={() => removeAttachedFile(index)}
                    className="ml-2 text-xs"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Input form */}
        <form
          onSubmit={handleSubmit}
          className="px-4 py-4 border-t flex items-center gap-2"
        >
          <span className="font-bold flex-shrink-0">$</span>
          <div className="flex-1 flex items-center gap-2 min-w-0">
            <Input
              variant="terminal"
              value={input}
              onChange={handleAgentInputChange}
              placeholder={isLoading ? 'agent busy...' : 'query agent...'}
              disabled={isLoading}
              className="flex-1 min-w-0"
              autoComplete="off"
              spellCheck="false"
            />

            <FileAttachment
              files={attachedFiles}
              onRemove={removeAttachedFile}
              onAdd={handleFileAttachment}
              compact={true}
            />
          </div>

          <Button
            type="submit"
            disabled={isLoading}
            variant="ghost"
            size="sm"
            className="px-1"
          >
            ⏎
          </Button>
        </form>
      </div>

      {/* Modals */}
      {showAssistantManager && <AssistantDetailList />}
      <ToolsModal
        isOpen={showToolsDetail}
        onClose={() => setShowToolsDetail(false)}
      />
      <ToolCaller />
      {children}
    </div>
  );
}
