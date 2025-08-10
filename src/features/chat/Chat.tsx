import TerminalHeader from '@/components/TerminalHeader';
import {
  Button,
  CompactModelPicker,
  FileAttachment,
  Input,
} from '@/components/ui';
import { useAssistantContext } from '@/context/AssistantContext';
import { useLocalTools } from '@/context/LocalToolContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext, ChatProvider } from '@/context/ChatContext';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { getLogger } from '@/lib/logger';
import { Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import ToolsModal from '../tools/ToolsModal';
import MessageBubble from './MessageBubble';
import { ToolCaller } from './orchestrators/ToolCaller';

const logger = getLogger('Chat');

interface ChatProps {
  children?: React.ReactNode;
}

// Main Chat container component
function Chat({ children }: ChatProps) {
  const [showToolsDetail, setShowToolsDetail] = useState(false);
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }

  return (
    <ChatProvider>
      <div className="h-full w-full font-mono flex flex-col rounded-lg overflow-hidden shadow-2xl">
        {children}
        <ToolCaller />
        <ToolsModal
          isOpen={showToolsDetail}
          onClose={() => setShowToolsDetail(false)}
        />
      </div>
    </ChatProvider>
  );
}

// Chat Header component
function ChatHeader({
  children,
  assistantName,
}: {
  children?: React.ReactNode;
  assistantName?: string;
}) {
  return (
    <TerminalHeader>
      {children}
      {assistantName && (
        <span className="ml-2 text-xs text-blue-400">[{assistantName}]</span>
      )}
    </TerminalHeader>
  );
}

// Chat Messages component
function ChatMessages() {
  const { messages, isLoading } = useChatContext();
  const { getCurrentSession, current: currentSession } = useSessionContext();
  const { getById } = useAssistantContext();

  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, messagesEndRef]);

  // 각 메시지의 assistantId로 이름을 찾아서 전달
  const getAssistantNameForMessage = useCallback(
    (m: Message) => {
      if (m.role === 'assistant' && 'assistantId' in m && m.assistantId) {
        const assistant = getById(m.assistantId);
        return assistant?.name || '';
      }
      // fallback: current session의 첫 assistant
      const currentSession = getCurrentSession();
      if (m.role === 'assistant' && currentSession?.assistants?.length) {
        return currentSession.assistants[0].name;
      }
      return '';
    },
    [getById, getCurrentSession],
  );

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 p-4 overflow-y-auto flex flex-col gap-6 terminal-scrollbar">
        {messages.map((m) => (
          <MessageBubble
            key={m.id}
            message={m}
            currentAssistantName={getAssistantNameForMessage(m)}
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
  );
}

// Chat Status Bar component - now manages its own tools state
function ChatStatusBar({
  children,
  onShowTools,
}: {
  children?: React.ReactNode;
  onShowTools?: () => void;
}) {
  // Move tools logic here since it's only used in this component
  const { availableTools: mcpTools } = useMCPServer();
  const { availableTools: localTools } = useLocalTools();
  const availableTools = useMemo(
    () => [...mcpTools, ...localTools],
    [mcpTools, localTools],
  );

  return (
    <div className="px-4 py-2 border-t flex items-center justify-between">
      <div>
        <CompactModelPicker />
        {children}
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs">Tools:</span>
        <button
          onClick={onShowTools}
          className="text-xs transition-colors flex items-center gap-1"
        >
          🔧 {availableTools.length} available
        </button>
      </div>
    </div>
  );
}

// Chat Attached Files component - now handles its own file removal
function ChatAttachedFiles() {
  const [attachedFiles, setAttachedFiles] = useState<
    { name: string; content: string }[]
  >([]);
  const removeAttachedFile = React.useCallback(
    (index: number) => {
      setAttachedFiles((prev) => prev.filter((_, i) => i !== index));
    },
    [setAttachedFiles],
  );

  if (attachedFiles.length === 0) return null;

  return (
    <div className="px-4 py-2 border-t">
      <div className="text-xs mb-2">📎 Attached Files:</div>
      <div className="flex flex-wrap gap-2">
        {attachedFiles.map((file, index) => (
          <div
            key={index}
            className="flex items-center px-2 py-1 rounded border border-gray-700"
          >
            <span className="text-xs truncate max-w-[150px]">{file.name}</span>
            <Button
              type="button"
              onClick={() => removeAttachedFile(index)}
              className="ml-2 text-xs"
            >
              ✕
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}

// Chat Input component - now handles its own file attachment logic
function ChatInput({ children }: { children?: React.ReactNode }) {
  const [input, setInput] = useState<string>('');
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { submit, isLoading } = useChatContext();

  const handleAgentInputChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setInput(e.target.value);
    },
    [setInput],
  );
  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      logger.info('submit!!', currentAssistant);
      e.preventDefault();
      if (!input.trim()) return;
      if (!currentAssistant) return;

      let messageContent = input.trim();

      const userMessage: Message = {
        id: createId(),
        content: messageContent,
        role: 'user',
        sessionId: currentSession?.id || '',
      };

      setInput('');

      try {
        await submit([userMessage]);
      } catch (err) {
        logger.error('Error submitting message:', err);
      }
    },
    [submit, input],
  );

  const handleFileAttachment = React.useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
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

      e.target.value = '';
    },
    [],
  );

  const removeAttachedFile = React.useCallback(() => {}, []);

  return (
    <form
      onSubmit={handleSubmit}
      className="px-4 py-4 border-t flex items-center gap-2"
    >
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <Input
          value={input}
          onChange={handleAgentInputChange}
          placeholder={isLoading ? 'agent busy...' : 'query agent...'}
          disabled={isLoading}
          className="flex-1 min-w-0"
          autoComplete="off"
          spellCheck="false"
        />

        <FileAttachment
          files={[]}
          onRemove={removeAttachedFile}
          onAdd={handleFileAttachment}
          compact={true}
        />
        {children}
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
  );
}

// Chat Bottom section (combines status bar, attached files, and input)
function ChatBottom({ children }: { children?: React.ReactNode }) {
  const [showToolsDetail, setShowToolsDetail] = useState(false);

  return (
    <div className="flex-shrink-0">
      <ChatStatusBar onShowTools={() => setShowToolsDetail(true)} />
      <ChatAttachedFiles />
      <ChatInput />
      <ToolsModal
        isOpen={showToolsDetail}
        onClose={() => setShowToolsDetail(false)}
      />
      {children}
    </div>
  );
}

// Attach subcomponents as static properties
Chat.Header = ChatHeader;
Chat.Messages = ChatMessages;
Chat.StatusBar = ChatStatusBar;
Chat.AttachedFiles = ChatAttachedFiles;
Chat.Input = ChatInput;
Chat.Bottom = ChatBottom;

export default Chat;