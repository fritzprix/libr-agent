import { useState, useCallback, useEffect } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useChatState');

export function useChatState() {
  const [showToolsDetail, setShowToolsDetail] = useState(false);
  const [showPlanningPanel, setShowPlanningPanel] = useState(false);

  // Component initialization logging
  useEffect(() => {
    logger.info('CHAT_STATE: Hook initialized', {
      showToolsDetail,
      showPlanningPanel,
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

  // State change logging
  useEffect(() => {
    logger.debug('CHAT_STATE: State updated', {
      showToolsDetail,
      showPlanningPanel,
    });
  }, [showToolsDetail, showPlanningPanel]);

  return {
    showToolsDetail,
    setShowToolsDetail: setShowToolsDetailWithLogging,
    showPlanningPanel,
    setShowPlanningPanel: setShowPlanningPanelWithLogging,
  };
}
