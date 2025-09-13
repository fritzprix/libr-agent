import { useState, useCallback, useEffect } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useChatState');

export function useChatState() {
  const [showToolsDetail, setShowToolsDetail] = useState(false);
  const [showPlanningPanel, setShowPlanningPanel] = useState(false);
  const [showWorkspacePanel, setShowWorkspacePanel] = useState(false);

  // Component initialization logging
  useEffect(() => {
    logger.info('CHAT_STATE: Hook initialized', {
      showToolsDetail,
      showPlanningPanel,
      showWorkspacePanel,
    });
  }, []);

  // Enhanced setters with logging
  const setShowToolsDetailWithLogging = useCallback(
    (value: boolean) => {
      logger.info('CHAT_STATE: Tools detail visibility changed', {
        from: showToolsDetail,
        to: value,
      });
      setShowToolsDetail(value);
    },
    [showToolsDetail],
  );

  const setShowPlanningPanelWithLogging = useCallback(
    (value: boolean) => {
      logger.info('CHAT_STATE: Planning panel visibility changed', {
        from: showPlanningPanel,
        to: value,
        action: value ? 'show' : 'hide',
      });
      setShowPlanningPanel(value);
    },
    [showPlanningPanel],
  );

  const setShowWorkspacePanelWithLogging = useCallback(
    (value: boolean) => {
      logger.info('CHAT_STATE: Workspace panel visibility changed', {
        from: showWorkspacePanel,
        to: value,
        action: value ? 'show' : 'hide',
      });
      setShowWorkspacePanel(value);
    },
    [showWorkspacePanel],
  );

  // State change logging
  useEffect(() => {
    logger.debug('CHAT_STATE: State updated', {
      showToolsDetail,
      showPlanningPanel,
      showWorkspacePanel,
    });
  }, [showToolsDetail, showPlanningPanel, showWorkspacePanel]);

  return {
    showToolsDetail,
    setShowToolsDetail: setShowToolsDetailWithLogging,
    showPlanningPanel,
    setShowPlanningPanel: setShowPlanningPanelWithLogging,
    showWorkspacePanel,
    setShowWorkspacePanel: setShowWorkspacePanelWithLogging,
  };
}
