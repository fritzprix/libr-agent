import { useChatContext } from '@/context/ChatContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useEffect } from 'react';
import { toast } from 'sonner';

function IdleDetector() {
  const { idle } = useScheduler();
  const { messages } = useChatContext();

  useEffect(() => {
    if (idle) {
      const lastMessage = messages[messages.length - 1];
      if (
        (lastMessage && lastMessage.isStreaming === false,
        lastMessage.role === 'assistant')
      ) {
        // TODO:
      }
    }
    toast.info(`New Idle State ${idle}`);
  }, [idle, messages]);

  return null;
}

export { IdleDetector };
