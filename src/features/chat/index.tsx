import { useSessionContext } from '@/context/SessionContext';
import Chat from './Chat';
import { Switch } from '@/components/ui/switch';
import { MultiAgentOrchestrator } from './orchestrators/MultiAgentOrchestrator';
import Reflection from './Reflection';

export function ChatRouter() {
  const { current, isAgenticMode, toggleAgenticMode } = useSessionContext();

  if (!current) {
    return null;
  }
  if (current.assistants.length > 1) {
    return (
      <Chat>
        <Chat.Header></Chat.Header>
        <Chat.Messages />
        <Chat.Bottom>
          <Chat.StatusBar />
          <Chat.AttachedFiles />
          <Chat.Input />
        </Chat.Bottom>
        <MultiAgentOrchestrator />
      </Chat>
    );
  }
  return (
    <Chat>
      <Chat.Header>
        <div className="flex items-center gap-2">
          <Switch
            id="agentic-mode-toggle"
            checked={isAgenticMode}
            onCheckedChange={toggleAgenticMode}
          />
          <label
            htmlFor="agentic-mode-toggle"
            className="text-sm font-medium select-none"
          >
            Agentic Mode
          </label>
        </div>
      </Chat.Header>
      <Chat.Messages />
      <Chat.Bottom>
        <Chat.StatusBar />
        <Chat.AttachedFiles />
        <Chat.Input />
      </Chat.Bottom>
      <Reflection />
    </Chat>
  );
}
