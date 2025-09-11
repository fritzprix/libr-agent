import React from 'react';
import TerminalHeader from '@/components/TerminalHeader';
import { useSessionContext } from '@/context/SessionContext';
import { useChatPlanning } from '../context/ChatPlanningContext';
import { SessionFilesPopover } from './SessionFilesPopover';
import { Button } from '@/components/ui/button';
import { PanelRight } from 'lucide-react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ChatHeader');

interface ChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function ChatHeader({ children, assistantName }: ChatHeaderProps) {
  const { current: currentSession } = useSessionContext();
  const { showPlanningPanel, togglePlanningPanel } = useChatPlanning();

  const handleTogglePlanningPanel = () => {
    logger.info('CHAT_HEADER: Planning panel toggle clicked', {
      currentState: showPlanningPanel,
      newState: !showPlanningPanel,
      sessionId: currentSession?.id,
      assistantName,
    });
    togglePlanningPanel();
  };

  return (
    <TerminalHeader>
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-blue-400">
              [{assistantName}]
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={handleTogglePlanningPanel}
            title="Toggle AI Planning Panel"
            className="h-6 px-2"
          >
            <PanelRight
              className={`h-4 w-4 ${showPlanningPanel ? 'text-blue-400' : ''}`}
            />
          </Button>

          {currentSession?.storeId && (
            <SessionFilesPopover storeId={currentSession.storeId} />
          )}
        </div>
      </div>
    </TerminalHeader>
  );
}
